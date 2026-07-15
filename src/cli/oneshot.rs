// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

mod container;

use crate::prelude::*;

use crate::eve;
use crate::geoip;
use crate::server::main::build_axum_service;
use crate::server::metrics::Metrics;
use crate::sqlite;
use crate::sqlite::configdb;
use clap::{CommandFactory, FromArgMatches, Parser};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::sync;

#[derive(Debug, Parser)]
#[command(
    name = "oneshot",
    about = "Import EVE JSON or process a PCAP and review it in EveBox"
)]
struct Args {
    // Hide the global data-directory option.
    #[arg(hide = true, global = true, short = 'D', long, name = "data-directory")]
    data_directory: Option<String>,

    /// Process INPUT as PCAP/PCAPNG with containerized Suricata (Linux only)
    #[arg(long)]
    pcap: bool,

    /// Container runtime: auto, podman, or docker (default: auto)
    #[arg(long, value_name = "RUNTIME", requires = "pcap")]
    container_runtime: Option<container::ContainerRuntimeChoice>,

    #[arg(
        long,
        value_name = "IMAGE",
        requires = "pcap",
        help = format!(
            "Suricata container image (default: {})",
            container::DEFAULT_SURICATA_IMAGE
        )
    )]
    suricata_image: Option<String>,

    /// Limit the number of events read
    #[arg(long, value_name = "limit")]
    limit: Option<u64>,

    /// Don't open browser
    #[arg(long)]
    no_open: bool,

    /// Don't wait for events to load
    #[arg(long)]
    no_wait: bool,

    /// Database filename
    #[arg(long, default_value = "./oneshot.sqlite", value_name = "FILENAME")]
    database_filename: String,

    /// Hostname/IP address to bind to
    #[arg(
        long,
        id = "http.host",
        default_value = "127.0.0.1",
        value_name = "HOSTNAME"
    )]
    host: String,

    #[arg(value_name = "INPUT")]
    input: PathBuf,
}

pub fn command() -> clap::Command {
    Args::command()
}

struct PreparedInput {
    eve_path: PathBuf,
    workspace: Option<TempDir>,
}

impl PreparedInput {
    fn eve(eve_path: PathBuf) -> Self {
        Self {
            eve_path,
            workspace: None,
        }
    }

    fn cleanup_path(&self) -> Option<PathBuf> {
        self.workspace.as_ref().map(|dir| dir.path().to_path_buf())
    }
}

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    let args = Args::from_arg_matches(args)?;
    let limit = args.limit.unwrap_or(0);
    let no_open = args.no_open;
    let no_wait = args.no_wait;
    let db_filename = args.database_filename.clone();
    let host = args.host.clone();
    let prepared_input = prepare_input(&args).await?;
    let input = prepared_input.eve_path.display().to_string();
    let generated_cleanup = prepared_input.cleanup_path();

    info!("Using database filename {}", &db_filename);

    let db_connection_builder = Arc::new(sqlite::ConnectionBuilder::filename(Some(
        &PathBuf::from(&db_filename),
    )));
    let mut conn = db_connection_builder.open_connection(true).await?;
    sqlite::connection::init_event_db(&mut conn).await?;
    let pool = sqlite::connection::open_pool(Some(&db_filename), false).await?;
    let db = crate::sqlite::connection::open_connection(Some(&db_filename), true).await?;
    let db = Arc::new(tokio::sync::Mutex::new(db));

    let metrics = Arc::new(Metrics::default());

    let mut import_task = {
        let metrics = metrics.clone();
        tokio::spawn(async move {
            let result = run_import(db, limit, &input, metrics).await;
            if let Err(err) = &result {
                error!("Import failure: {:#}", err);
            }
            drop(prepared_input);
            result
        })
    };

    if !no_wait {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Got CTRL-C, will start server now. Hit CTRL-C again to exit");
            }
            result = &mut import_task => {
                result.context("oneshot import task failed")??;
            }
        }
    }
    let (port_tx, mut port_rx) = sync::mpsc::unbounded_channel::<u16>();

    // Initialize config repo.
    let config_repo = configdb::open(None).await?;

    let server_cleanup = generated_cleanup.clone();
    let server = {
        let host = host.clone();
        let metrics = metrics.clone();
        tokio::spawn(async move {
            let mut port = 5636;
            loop {
                let conn = Arc::new(tokio::sync::Mutex::new(
                    db_connection_builder.open_connection(false).await.unwrap(),
                ));
                let sqlite_datastore =
                    sqlite::eventrepo::SqliteEventRepo::new(conn, pool.clone(), metrics.clone());
                let ds = crate::eventrepo::EventRepo::SQLite(sqlite_datastore);
                let config = crate::server::ServerConfig {
                    port,
                    host: host.clone(),
                    elastic_url: "".to_string(),
                    elastic_index: "".to_string(),
                    no_check_certificate: false,
                    datastore: "sqlite".to_string(),
                    ..crate::server::ServerConfig::default()
                };

                let context = match crate::server::build_context(
                    config.clone(),
                    ds,
                    config_repo.clone(),
                    Arc::new(Metrics::default()),
                )
                .await
                {
                    Ok(mut context) => {
                        context.mode = crate::server::ServerMode::Oneshot;
                        context.defaults.time_range = Some("all".to_string());
                        Arc::new(context)
                    }
                    Err(err) => {
                        error!("Failed to build server context: {}", err);
                        cleanup_generated(&server_cleanup);
                        std::process::exit(1);
                    }
                };
                debug!("Successfully build server context");

                match tokio::net::TcpListener::bind(&format!("{}:{}", config.host, port)).await {
                    Ok(listener) => {
                        let service = build_axum_service(context);
                        let server = axum::serve(listener, service);
                        port_tx.send(port).unwrap();
                        server.await.unwrap();
                        break;
                    }
                    Err(_) => {
                        warn!(
                            "Failed to start server on port {}, will try {}",
                            port,
                            port + 1
                        );
                        port += 1;
                    }
                }
            }
        })
    };

    let port = port_rx.recv().await.unwrap();
    let url = format!("http://{host}:{port}");
    info!("Server started at {}", url);

    let connect_url = if host == "0.0.0.0" {
        format!("http://127.0.0.1:{port}")
    } else {
        format!("http://{host}:{port}")
    };

    if !no_open {
        if let Err(err) = webbrowser::open(&connect_url) {
            error!("Failed to open {} in browser: {}", url, err);
        }

        info!(
            "If your browser didn't open, try connecting to {}",
            connect_url
        );
    }

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        info!("Got CTRL-C, exiting");
        cleanup_database(&db_filename);
        cleanup_generated(&generated_cleanup);
        std::process::exit(0);
    });

    server.await?;
    Ok(())
}

async fn prepare_input(args: &Args) -> Result<PreparedInput> {
    let input = args.input.clone();
    if !args.pcap {
        return Ok(PreparedInput::eve(input));
    }

    ensure_pcap_supported(std::env::consts::OS)?;
    let pcap = validate_pcap(&input)?;
    let workspace = tempfile::Builder::new()
        .prefix("evebox-oneshot-pcap-")
        .tempdir()
        .context("failed to create private PCAP processing workspace")?;
    let staged_pcap = stage_pcap(&pcap, workspace.path())?;
    let eve_path = workspace.path().join("eve.json");
    let runtime = args.container_runtime.unwrap_or_default();
    let image = args
        .suricata_image
        .as_deref()
        .unwrap_or(container::DEFAULT_SURICATA_IMAGE);

    container::generate_eve(&staged_pcap, &eve_path, runtime, image).await?;
    if let Err(err) = std::fs::remove_file(&staged_pcap) {
        warn!(
            "Failed to remove staged PCAP {}: {}",
            staged_pcap.display(),
            err
        );
    }
    Ok(PreparedInput {
        eve_path,
        workspace: Some(workspace),
    })
}

fn stage_pcap(pcap: &Path, workspace: &Path) -> Result<PathBuf> {
    let staged_pcap = workspace.join("capture.pcap");
    std::fs::copy(pcap, &staged_pcap).with_context(|| {
        format!(
            "failed to stage PCAP input {} in the private processing workspace",
            pcap.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged_pcap, std::fs::Permissions::from_mode(0o444))
            .context("failed to make the staged PCAP read-only")?;
    }
    Ok(staged_pcap)
}

fn ensure_pcap_supported(platform: &str) -> Result<()> {
    if platform != "linux" {
        anyhow::bail!("oneshot PCAP processing is currently supported on Linux only");
    }
    Ok(())
}

fn validate_pcap(path: &Path) -> Result<PathBuf> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("failed to access PCAP input {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("PCAP input is not a regular file: {}", path.display());
    }

    let mut file = File::open(path)
        .with_context(|| format!("failed to open PCAP input {}", path.display()))?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)
        .with_context(|| format!("failed to read PCAP input {}", path.display()))?;
    const PCAP_MAGICS: [[u8; 4]; 5] = [
        [0xd4, 0xc3, 0xb2, 0xa1],
        [0xa1, 0xb2, 0xc3, 0xd4],
        [0x4d, 0x3c, 0xb2, 0xa1],
        [0xa1, 0xb2, 0x3c, 0x4d],
        [0x0a, 0x0d, 0x0d, 0x0a],
    ];
    if !PCAP_MAGICS.contains(&magic) {
        anyhow::bail!(
            "input is not an uncompressed PCAP or PCAPNG file: {}",
            path.display()
        );
    }

    path.canonicalize()
        .with_context(|| format!("failed to resolve PCAP input {}", path.display()))
}

fn cleanup_database(filename: &str) {
    let _ = std::fs::remove_file(filename);
    let _ = std::fs::remove_file(format!("{}-shm", filename));
    let _ = std::fs::remove_file(format!("{}-wal", filename));
}

fn cleanup_generated(path: &Option<PathBuf>) {
    if let Some(path) = path
        && let Err(err) = std::fs::remove_dir_all(path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        warn!(
            "Failed to remove PCAP processing workspace {}: {}",
            path.display(),
            err
        );
    }
}

async fn run_import(
    sqlx: Arc<tokio::sync::Mutex<sqlx::SqliteConnection>>,
    limit: u64,
    input: &str,
    metrics: Arc<crate::server::metrics::Metrics>,
) -> anyhow::Result<()> {
    let geoipdb = geoip::GeoIP::open(None).ok();
    let mut indexer = sqlite::importer::SqliteEventSink::new(sqlx, metrics);
    let mut reader = eve::reader::EveReader::new(input.into());
    info!("Reading {} ({} bytes)", input, reader.file_size());
    let mut last_percent = 0;
    let mut count = 0;
    let start = std::time::Instant::now();
    while let Some(mut next) = reader.next_file_record()? {
        if let Some(geoipdb) = &geoipdb {
            geoipdb.add_geoip_to_eve(&mut next);
        }
        indexer.submit(next).await?;
        count += 1;
        let size = reader.file_size();
        let offset = reader.offset();
        let pct = if size == 0 {
            100
        } else {
            ((offset as f64 / size as f64) * 100.0) as u64
        };
        if pct != last_percent {
            info!("{}: {} events ({}%)", input, count, pct);
            last_percent = pct;
        }
        if indexer.pending() > 300 {
            indexer.commit().await?;
        }
        if limit > 0 && count == limit {
            break;
        }
    }
    indexer.commit().await?;
    let elapsed = start.elapsed();
    info!("Read {} events in {}s", count, elapsed.as_secs_f64());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;
    use std::io::Write;

    #[test]
    fn pcap_options_default_in_implementation() {
        let args = Args::try_parse_from(["oneshot", "capture.pcap"]).unwrap();
        assert!(!args.pcap);
        assert!(args.container_runtime.is_none());
        assert_eq!(
            container::ContainerRuntimeChoice::default(),
            container::ContainerRuntimeChoice::Auto
        );
        assert_eq!(
            container::DEFAULT_SURICATA_IMAGE,
            "docker.io/jasonish/suricata:8.0"
        );
    }

    #[test]
    fn container_options_require_pcap_mode() {
        for option in [
            ["oneshot", "--container-runtime", "docker", "eve.json"],
            [
                "oneshot",
                "--suricata-image",
                "example/suricata",
                "eve.json",
            ],
        ] {
            let err = Args::try_parse_from(option).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        }
    }

    #[test]
    fn explicit_runtime_is_parsed() {
        let args = Args::try_parse_from([
            "oneshot",
            "--pcap",
            "--container-runtime",
            "docker",
            "--suricata-image",
            "example/suricata:8",
            "capture.pcap",
        ])
        .unwrap();
        assert_eq!(
            args.container_runtime,
            Some(container::ContainerRuntimeChoice::Docker)
        );
        assert_eq!(args.suricata_image.as_deref(), Some("example/suricata:8"));
    }

    #[test]
    fn pcap_mode_rejects_non_linux_platforms() {
        let err = ensure_pcap_supported("windows").unwrap_err();
        assert!(err.to_string().contains("Linux only"));
    }

    #[test]
    fn validates_pcap_and_pcapng_magic() {
        for magic in [[0xd4, 0xc3, 0xb2, 0xa1], [0x0a, 0x0d, 0x0d, 0x0a]] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("capture file");
            let mut file = File::create(&path).unwrap();
            file.write_all(&magic).unwrap();
            assert_eq!(validate_pcap(&path).unwrap(), path.canonicalize().unwrap());
        }
    }

    #[test]
    fn rejects_non_pcap_input() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"{\"event_type\":\"alert\"}\n").unwrap();
        let err = validate_pcap(file.path()).unwrap_err();
        assert!(err.to_string().contains("not an uncompressed PCAP"));
    }

    #[cfg(unix)]
    #[test]
    fn stages_owner_only_pcap_without_changing_source_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let source_dir = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("private.pcap");
        std::fs::write(&source, b"pcap contents").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600)).unwrap();

        let staged = stage_pcap(&source, workspace.path()).unwrap();
        assert_eq!(std::fs::read(&staged).unwrap(), b"pcap contents");
        assert_eq!(
            std::fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&staged).unwrap().permissions().mode() & 0o777,
            0o444
        );
    }

    #[tokio::test]
    async fn existing_eve_input_needs_no_workspace() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let args = Args::try_parse_from([
            "oneshot",
            file.path().to_str().unwrap(),
            "--limit",
            "1",
            "--no-wait",
            "--no-open",
            "--database-filename",
            "custom.sqlite",
        ])
        .unwrap();
        let prepared = prepare_input(&args).await.unwrap();
        assert_eq!(prepared.eve_path, file.path());
        assert!(prepared.workspace.is_none());
    }

    async fn import_file(input: &Path) -> Result<()> {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("events.sqlite");
        let mut connection = sqlite::connection::open_connection(Some(&database), true).await?;
        sqlite::connection::init_event_db(&mut connection).await?;
        run_import(
            Arc::new(tokio::sync::Mutex::new(connection)),
            0,
            input.to_str().unwrap(),
            Arc::new(Metrics::default()),
        )
        .await
    }

    #[tokio::test]
    async fn import_propagates_file_open_errors() {
        let path = PathBuf::from("/definitely/missing/evebox-oneshot-eve.json");
        let err = import_file(&path).await.unwrap_err();
        assert!(err.to_string().contains("io error"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn import_propagates_file_read_errors() {
        let directory = tempfile::tempdir().unwrap();
        let err = import_file(directory.path()).await.unwrap_err();
        assert!(err.to_string().contains("io error"));
    }

    #[tokio::test]
    async fn import_propagates_malformed_json() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"{not-json}").unwrap();
        let err = import_file(file.path()).await.unwrap_err();
        assert!(err.to_string().contains("failed to parse event on line 1"));
    }

    #[tokio::test]
    async fn import_accepts_valid_eve_json() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            b"{\"timestamp\":\"2026-07-15T12:00:00.000000+0000\",\"event_type\":\"stats\"}",
        )
        .unwrap();
        import_file(file.path()).await.unwrap();
    }
}
