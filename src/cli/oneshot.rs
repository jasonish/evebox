// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::config::Config;
use crate::eve;
use crate::geoip;
use crate::server::main::build_axum_service;
use crate::sqlite;
use crate::sqlite::configrepo;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    let config_loader = Config::new(args.clone(), None)?;
    let limit: u64 = config_loader.get("limit")?.unwrap_or(0);
    let no_open: bool = config_loader.get_bool("no-open")?;
    let no_wait: bool = config_loader.get_bool("no-wait")?;
    let db_filename: String = config_loader.get("database-filename")?.unwrap();
    let host: String = config_loader.get("http.host")?.unwrap();
    let input = args.get_one::<String>("INPUT").unwrap().to_string();

    info!("Using database filename {}", &db_filename);

    let db_connection_builder = Arc::new(sqlite::ConnectionBuilder::filename(Some(
        &PathBuf::from(&db_filename),
    )));
    let mut conn = db_connection_builder.open_connection(true).await?;
    sqlite::connection::init_event_db(&mut conn).await?;
    let pool = sqlite::connection::open_pool(Some(&db_filename), false).await?;
    let db = crate::sqlite::connection::open_connection(Some(&db_filename), true).await?;
    let db = Arc::new(tokio::sync::Mutex::new(db));

    let import_task = {
        tokio::spawn(async move {
            if let Err(err) = run_import(db, limit, &input).await {
                error!("Import failure: {}", err);
            }
        })
    };

    if !no_wait {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Got CTRL-C, will start server now. Hit CTRL-C again to exit");
            }
            _ = import_task => {}
        }
    }
    let (port_tx, mut port_rx) = sync::mpsc::unbounded_channel::<u16>();

    // Initialize config repo.
    let config_repo = configrepo::open(None).await?;

    let server = {
        let host = host.clone();
        tokio::spawn(async move {
            let mut port = 5636;
            loop {
                let conn = Arc::new(tokio::sync::Mutex::new(
                    db_connection_builder.open_connection(false).await.unwrap(),
                ));
                let sqlite_datastore = sqlite::eventrepo::SqliteEventRepo::new(conn, pool.clone());
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

                let context =
                    match crate::server::build_context(config.clone(), ds, config_repo.clone())
                        .await
                    {
                        Ok(mut context) => {
                            context.defaults.time_range = Some("all".to_string());
                            Arc::new(context)
                        }
                        Err(err) => {
                            error!("Failed to build server context: {}", err);
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
        let _ = std::fs::remove_file(&db_filename);
        let _ = std::fs::remove_file(format!("{}-shm", &db_filename));
        let _ = std::fs::remove_file(format!("{}-wal", &db_filename));
        std::process::exit(0);
    });

    server.await?;
    Ok(())
}

async fn run_import(
    sqlx: Arc<tokio::sync::Mutex<sqlx::SqliteConnection>>,
    limit: u64,
    input: &str,
) -> anyhow::Result<()> {
    let geoipdb = match geoip::GeoIP::open(None) {
        Ok(geoipdb) => Some(geoipdb),
        Err(_) => None,
    };
    let mut indexer = sqlite::importer::SqliteEventSink::new(sqlx);
    let mut reader = eve::reader::EveReader::new(input.into());
    info!("Reading {} ({} bytes)", input, reader.file_size());
    let mut last_percent = 0;
    let mut count = 0;
    let start = std::time::Instant::now();
    loop {
        match reader.next_record() {
            Ok(None) | Err(_) => {
                break;
            }
            Ok(Some(mut next)) => {
                if let Some(geoipdb) = &geoipdb {
                    geoipdb.add_geoip_to_eve(&mut next);
                }
                indexer.submit(next).await?;
                count += 1;
                let size = reader.file_size();
                let offset = reader.offset();
                let pct = ((offset as f64 / size as f64) * 100.0) as u64;
                if pct != last_percent {
                    info!("{}: {} events ({}%)", input, count, pct);
                    last_percent = pct;
                }
                if indexer.pending() > 300 {
                    indexer.commit().await?;
                }
            }
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
