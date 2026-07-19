// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::agent::client::Client;
use crate::agent::importer::EveBoxEventSink;
use crate::config::Config;
use crate::eve::filters::EveFilterChain;
use crate::importer::EventSink;
use crate::{bookmark, eve};
use clap::{CommandFactory, Parser};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

#[derive(Parser, Debug)]
#[command(name = "agent", about = "EveBox Agent")]
struct Args {
    /// Agent configuration filename
    #[arg(short, long)]
    config: Option<String>,

    /// EveBox Server or Elasticsearch URL
    #[arg(long, id = "server.url", value_name = "URL")]
    server: Option<String>,

    /// Enable GeoIP
    #[arg(long = "enable-geoip", id = "geoip.enabled")]
    geoip: bool,

    /// Bookmark directory (deprecated).
    #[arg(long, id = "bookmark-directory", hide(true))]
    bookmark_directory: Option<String>,

    #[arg(from_global, id = "data-directory")]
    data_directory: Option<String>,

    /// Submit events to Elasticsearch instead of EveBox.
    #[arg(
        short,
        long,
        id = "elasticsearch.enabled",
        env = "EVEBOX_ELASTICSEARCH_ENABLED",
        hide_env(true)
    )]
    elasticsearch: bool,

    /// Elasticsearch URL
    #[arg(
        long,
        id = "elasticsearch.url",
        value_name = "URL",
        default_value = "http://localhost:9200",
        env = "EVEBOX_ELASTICSEARCH_URL",
        hide_env(true)
    )]
    elasticsearch_url: String,

    /// Elasticsearch index
    #[arg(
        long,
        default_value = "logstash",
        value_name = "NAME",
        id = "elasticsearch.index",
        env = "EVEBOX_ELASTICSEARCH_INDEX",
        hide_env(true)
    )]
    elasticsearch_index: String,

    /// Don't use an Elasticsearch index date suffix.
    #[arg(
        long,
        id = "elasticsearch.nodate",
        env = "EVEBOX_ELASTICSEARCH_NODATE",
        hide_env(true)
    )]
    elasticsearch_nodate: bool,

    /// Disable TLS certificate checks.
    #[arg(long, short = 'k', id = "disable-certificate-check", aliases = &["no-certificate-check"])]
    disable_certificate_check: bool,

    /// Unique agent identifier, advertised on the control channel
    #[arg(long, id = "agent-id", value_name = "ID")]
    agent_id: Option<String>,

    /// Enable full packet capture from this Suricata pcap-log spool directory
    #[arg(long, id = "pcap.directory", value_name = "DIR")]
    pcap_directory: Option<String>,

    /// Filename prefix of the pcap spool files
    #[arg(long, id = "pcap.prefix", value_name = "PREFIX")]
    pcap_prefix: Option<String>,

    /// Log file names/patterns to process
    filenames: Vec<String>,
}

pub fn command() -> clap::Command {
    Args::command()
}

pub async fn main(args_matches: &clap::ArgMatches) -> anyhow::Result<()> {
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        std::process::exit(0);
    });

    let config_filename = match args_matches.get_one::<String>("config").map(|s| s.as_str()) {
        Some(v) => Some(v),
        None => find_config_filename(),
    };
    if let Some(filename) = config_filename {
        debug!("Using configuration file {}", filename);
    }
    let config = Config::new(args_matches.clone(), config_filename)?;

    let server_url = config
        .get_string("server.url")
        .unwrap_or_else(|| "http://localhost:5636".to_string());
    let server_username = config.get_string("server.username");
    let server_password = config.get_string("server.password");
    let disable_certificate_check = config
        .get_bool("disable-certificate-check")
        .unwrap_or(false);

    // One identity for everything this agent does: stamped on imported
    // events and claimed on the packet-capture control channel, so the
    // server can route an event's capture request back to this agent.
    #[cfg(not(windows))]
    let (agent_id, agent_hostname) = agent_identity(&config);
    #[cfg(windows)]
    let (agent_id, _) = agent_identity(&config);

    // The packet-capture channel is optional and deliberately independent of
    // the EVE importer tasks below. Direct-to-Elasticsearch mode has no
    // EveBox server connection to carry control messages, so it cannot serve
    // remote capture requests.
    #[cfg(not(windows))]
    let pcap_channel = build_pcap_channel(
        &config,
        &server_url,
        disable_certificate_check,
        &agent_id,
        &agent_hostname,
    )?;
    #[cfg(windows)]
    let pcap_channel = {
        if config
            .get_string("pcap.directory")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        {
            warn!("Full packet capture is not supported on Windows; ignoring pcap configuration");
        }
        None::<()>
    };

    // Collect eve filenames.
    let eve_filenames = get_eve_filenames(&config)?;
    if eve_filenames.is_empty() {
        if pcap_channel.is_some() {
            info!("No EVE log files provided; running in pcap-only mode (events are not shipped)");
        } else {
            bail!("No EVE log files provided. Exiting as there is nothing to do.");
        }
    }

    let enable_geoip = args_matches
        .get_one::<bool>("geoip.enabled")
        .is_some_and(|v| *v);

    let rule_filenames = get_rule_filenames(&config)?;

    let mut filters = EveFilterChain::with_defaults();
    filters.add_filter(eve::filters::AddAgentHostnameFilter::default());
    filters.add_filter(eve::filters::AddAgentIdFilter::new(agent_id));

    if enable_geoip {
        match crate::geoip::GeoIP::open(None) {
            Err(err) => {
                warn!("Failed to open GeoIP database: {}", err);
            }
            Ok(geoipdb) => {
                filters.add_filter(eve::filters::GeoIpFilter::new(geoipdb));
            }
        }
    }

    if !rule_filenames.is_empty() {
        let rule_collection = Arc::new(crate::rules::load_rules(&rule_filenames));
        filters.add_filter(crate::eve::filters::AddRuleFilter::new(
            rule_collection.clone(),
        ));
        crate::rules::watch_rules(rule_collection);
    }

    // Get additional fields to add to events.
    let additional_fields = get_additional_fields(&config)?;
    if let Some(custom_fields) = additional_fields {
        for (field, value) in custom_fields {
            info!("Adding custom field: {} -> {:?}", field, value);
            let filter = crate::eve::filters::AddFieldFilter::new(field, value);
            filters.add_filter(filter);
        }
    }

    let mut log_runners: HashMap<String, bool> = HashMap::new();

    let importer = if config.get_bool("elasticsearch.enabled")? {
        let url = config.get_string("elasticsearch.url").unwrap();
        let mut client = crate::elastic::ClientBuilder::new(&url);
        client = client.disable_certificate_validation(disable_certificate_check);
        if let Some(username) = config.get_string("elasticsearch.username") {
            client = client.with_username(&username);
        }
        if let Some(password) = config.get_string("elasticsearch.password") {
            client = client.with_password(&password);
        }
        let nodate = config.get_bool("elasticsearch.nodate")?;
        let index = config.get_string("elasticsearch.index").unwrap();
        let client = client.build();
        // Detect OpenSearch so stats events can be split into their own index
        // (OpenSearch only). Best effort: fall back to Elasticsearch behaviour
        // if the server can't be reached yet.
        let opensearch = client
            .get_info()
            .await
            .ok()
            .and_then(|info| info.version.distribution)
            .as_deref()
            == Some("opensearch");
        info!(
            "Sending events to Elasticsearch: {url}, index={index}, nodate={nodate}, opensearch={opensearch}"
        );
        let importer =
            crate::elastic::importer::ElasticEventSink::new(client, &index, nodate, opensearch);
        EventSink::Elastic(importer)
    } else {
        let client = Client::new(
            &server_url,
            server_username,
            server_password,
            disable_certificate_check,
        );
        info!("Sending events to EveBox server: {server_url}");
        EventSink::EveBox(EveBoxEventSink::new(client))
    };

    let bookmark_directory = config.get_string("bookmark-directory");
    if bookmark_directory.is_some() {
        warn!("Found deprecated option bookmark-directory, please use data-directory");
    }
    let data_directory = config.get_string("data-directory");
    if let Some(directory) = &data_directory {
        debug!("Using data-directory {}", directory);
    }

    let bookmark_directory = if bookmark_directory.is_some() {
        bookmark_directory
    } else {
        data_directory
    };

    // This forever-retrying task owns its own lifecycle. Keep it outside the
    // fail-fast EVE processor set so a control-channel reconnect can never
    // terminate event shipping, and pcap-only mode can have an empty set.
    #[cfg(not(windows))]
    if let Some(channel) = pcap_channel {
        info!("Starting full packet capture control channel");
        tokio::spawn(crate::agent::channel::run(channel));
    }

    let mut tasks = FuturesUnordered::new();

    loop {
        for path in &eve_filenames {
            for path in crate::path::expand(path)? {
                let path = path.display().to_string();
                if !log_runners.contains_key(&path) {
                    info!("Found EVE log file {:?}", &path);
                    log_runners.insert(path.clone(), true);
                    let task = start_runner(
                        &path,
                        importer.clone(),
                        bookmark_directory.clone(),
                        filters.clone(),
                    );
                    tasks.push(task);
                }
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            _ = tasks.select_next_some(), if !tasks.is_empty() => {
                bail!("A log processing task unexpectedly aborted");
            }
        }
    }
}

/// The identity this agent presents everywhere: the configured `agent-id`
/// (or the system hostname when unset) plus the hostname itself.
fn agent_identity(config: &Config) -> (String, String) {
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    let agent_id = config
        .get_string("agent-id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| hostname.clone());
    (agent_id, hostname)
}

/// Build the persistent packet-capture channel configuration, or return
/// `None` when no spool directory is configured or packet capture is
/// incompatible with the selected output.
#[cfg(not(windows))]
fn build_pcap_channel(
    config: &Config,
    server_url: &str,
    disable_certificate_check: bool,
    agent_id: &str,
    hostname: &str,
) -> anyhow::Result<Option<crate::agent::channel::ChannelConfig>> {
    // Packet capture is enabled by setting a spool directory, either
    // `pcap.directory` in the configuration file or --pcap-directory on the
    // command line; there is no separate enable flag, matching the server.
    let Some(directory) = config
        .get_string("pcap.directory")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    if config.get_bool("elasticsearch.enabled")? {
        warn!(
            "Full packet capture is not supported with direct Elasticsearch output; ignoring pcap configuration"
        );
        return Ok(None);
    }
    let prefix = config
        .get_string("pcap.prefix")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let server_key = config
        .get_string("server.key")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let server_url = crate::agent::tls::normalize_server_url(server_url)?;
    info!("Full packet capture enabled: spool {directory} as agent {agent_id:?}");

    Ok(Some(crate::agent::channel::ChannelConfig {
        server_url,
        agent_id: agent_id.to_string(),
        hostname: hostname.to_string(),
        server_key,
        spool: crate::pcap::SpoolConfig::new(directory, prefix),
        disable_certificate_check,
    }))
}

fn start_runner(
    filename: &str,
    importer: EventSink,
    bookmark_directory: Option<String>,
    mut filters: EveFilterChain,
) -> JoinHandle<()> {
    let mut end = false;
    let reader = crate::eve::reader::EveReader::new(filename.into());
    let bookmark_filename = get_bookmark_filename(filename, bookmark_directory);
    if let Some(bookmark_filename) = &bookmark_filename {
        info!("Using bookmark file: {:?}", bookmark_filename);
    } else {
        warn!("Failed to determine usable bookmark filename, will start reading at end of file");
        end = true;
    }
    let mut processor = crate::eve::Processor::new(reader, importer);
    processor.end = end;

    filters.add_filter(eve::filters::AddAgentFilenameFilter::new(
        filename.to_string(),
    ));

    processor.filter_chain = Some(filters);
    processor.report_interval = std::time::Duration::from_secs(60);
    processor.bookmark_filename = bookmark_filename;
    tokio::spawn(async move {
        processor.run().await;
    })
}

fn find_config_filename() -> Option<&'static str> {
    let paths = ["./agent.yaml", "/etc/evebox/agent.yaml"];
    for path in paths {
        debug!("Checking for {}", path);
        let pathbuf = PathBuf::from(path);
        if pathbuf.exists() {
            return Some(path);
        }
    }
    None
}

fn get_additional_fields(
    config: &Config,
) -> anyhow::Result<Option<HashMap<String, serde_json::Value>>> {
    let additional_fields: Option<HashMap<String, serde_yaml::Value>> =
        config.get_value("additional-fields")?;
    if let Some(fields) = &additional_fields {
        // Convert to JSON.
        let fields: HashMap<String, serde_json::Value> =
            serde_json::from_str(&serde_json::to_string(&fields)?)?;
        Ok(Some(fields))
    } else {
        Ok(None)
    }
}

fn get_eve_filenames(config: &Config) -> anyhow::Result<Vec<String>> {
    let mut eve_filenames: Vec<String> = vec![];

    if config.args.contains_id("filenames") {
        eve_filenames.extend(
            config
                .args
                .get_many::<String>("filenames")
                .unwrap()
                .map(String::from)
                .collect::<Vec<String>>(),
        );
    } else {
        match config.get_value::<Vec<String>>("input.paths") {
            Ok(Some(filenames)) => {
                eve_filenames.extend(filenames);
            }
            Ok(None) => {}
            Err(_) => {
                bail!("There was an error reading 'input.paths' from the configuration file");
            }
        }

        // Also use input.filename.
        if let Ok(Some(filename)) = config.get_value::<String>("input.filename") {
            eve_filenames.push(filename);
        }
    }

    Ok(eve_filenames)
}

fn get_rule_filenames(config: &Config) -> anyhow::Result<Vec<String>> {
    match config.get_value::<Vec<String>>("rules") {
        Ok(Some(filenames)) => Ok(filenames),
        Ok(None) => {
            // No `rules` found, check `input.rules`.
            match config.get_value::<Vec<String>>("input.rules") {
                Ok(Some(filenames)) => {
                    warn!("Found rule filenames in deprecated configuration section 'input.rules'");
                    Ok(filenames)
                }
                Ok(None) => Ok(vec![]),
                Err(_) => {
                    bail!("There was an error reading 'input.rules' from the configuration file");
                }
            }
        }
        Err(_) => {
            bail!("There was an error reading 'rules' from the configuration file");
        }
    }
}

fn get_bookmark_filename(input: &str, directory: Option<String>) -> Option<PathBuf> {
    if let Some(directory) = directory {
        return Some(bookmark::bookmark_filename(input, &directory));
    } else {
        let filename = PathBuf::from(format!("{input}.bookmark"));

        if filename.exists() {
            info!(
                "Legacy bookmark filename exists, will check if writable: {:?}",
                &filename
            );
            if let Err(err) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&filename)
            {
                warn!(
                    "Failed open deprecated bookmark file {:?}, will not use: {}",
                    &filename, err
                );
            } else {
                info!("Using deprecated bookmark file {:?}", &filename);
                return Some(filename);
            }
        }

        let filename = bookmark::bookmark_filename(input, ".");
        info!("Testing bookmark filename {:?}", filename);
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filename)
        {
            Ok(_) => {
                if let Ok(meta) = std::fs::metadata(&filename)
                    && meta.len() == 0
                {
                    let _ = std::fs::remove_file(&filename);
                }
                info!("Bookmark file {:?} looks OK", filename);
                return Some(filename);
            }
            Err(err) => {
                warn!("Error using {:?} as bookmark filename: {}", filename, err);
            }
        }
    }
    None
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    fn yaml_config(yaml: &str) -> (tempfile::TempDir, Config) {
        yaml_config_with_args(yaml, &[])
    }

    /// Build the channel the way `main()` does: the agent identity is
    /// resolved from the same configuration.
    fn channel_from(
        config: &Config,
        server_url: &str,
    ) -> anyhow::Result<Option<crate::agent::channel::ChannelConfig>> {
        let (agent_id, hostname) = agent_identity(config);
        build_pcap_channel(config, server_url, false, &agent_id, &hostname)
    }

    fn yaml_config_with_args(yaml: &str, args: &[&str]) -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.yaml");
        std::fs::write(&path, yaml).unwrap();
        let argv: Vec<&str> = std::iter::once("agent")
            .chain(args.iter().copied())
            .collect();
        let matches = Args::command().get_matches_from(argv);
        let config = Config::new(matches, path.to_str()).unwrap();
        (dir, config)
    }

    #[test]
    fn pcap_channel_reads_spool_and_agent_identity() {
        let (dir, config) = yaml_config(
            "elasticsearch:\n  enabled: false\nagent-id: edge-a\npcap:\n  directory: /captures\n  prefix: '  log.pcap  '\n",
        );
        let channel = channel_from(&config, "https://evebox.test")
            .unwrap()
            .unwrap();
        assert_eq!(channel.agent_id, "edge-a");
        assert_eq!(channel.server_url, "https://evebox.test");
        assert_eq!(channel.spool.directory, PathBuf::from("/captures"));
        assert_eq!(channel.spool.prefix.as_deref(), Some("log.pcap"));
        drop(dir);
    }

    #[test]
    fn command_line_agent_id_overrides_configuration_file() {
        let (_dir, config) = yaml_config_with_args(
            "elasticsearch:\n  enabled: false\nagent-id: from-yaml\npcap:\n  directory: /captures\n",
            &["--agent-id", "suri-9"],
        );
        let channel = channel_from(&config, "https://evebox.test")
            .unwrap()
            .unwrap();
        assert_eq!(channel.agent_id, "suri-9");
    }

    #[test]
    fn blank_directory_disables_channel_and_other_strings_are_normalized() {
        let (_dir, blank_directory) =
            yaml_config("elasticsearch:\n  enabled: false\npcap:\n  directory: '   '\n");
        assert!(
            channel_from(&blank_directory, "http://evebox.test")
                .unwrap()
                .is_none()
        );

        let (_dir, blank_agent_id) = yaml_config(
            "elasticsearch:\n  enabled: false\nagent-id: '   '\npcap:\n  directory: '  /captures  '\n  prefix: '   '\n",
        );
        let channel = channel_from(
            &blank_agent_id,
            "https://evebox.test/base///?ignored=yes#fragment",
        )
        .unwrap()
        .unwrap();
        assert_eq!(channel.spool.directory, PathBuf::from("/captures"));
        assert_eq!(channel.spool.prefix, None);
        assert_eq!(
            channel.agent_id,
            gethostname::gethostname().to_string_lossy()
        );
        assert_eq!(channel.server_url, "https://evebox.test/base");
    }

    #[test]
    fn invalid_pcap_server_url_fails_channel_configuration() {
        let (_dir, config) =
            yaml_config("elasticsearch:\n  enabled: false\npcap:\n  directory: /captures\n");
        assert!(channel_from(&config, "ws://evebox.test").is_err());
        assert!(channel_from(&config, "ftp://evebox.test").is_err());
    }

    #[test]
    fn command_line_pcap_directory_starts_channel() {
        // No pcap block in the configuration file at all.
        let (_dir, config) = yaml_config_with_args(
            "elasticsearch:\n  enabled: false\n",
            &["--pcap-directory", "/captures", "--pcap-prefix", "log.pcap"],
        );
        let channel = channel_from(&config, "https://evebox.test")
            .unwrap()
            .unwrap();
        assert_eq!(channel.spool.directory, PathBuf::from("/captures"));
        assert_eq!(channel.spool.prefix.as_deref(), Some("log.pcap"));
    }

    #[test]
    fn command_line_pcap_directory_overrides_configuration_file() {
        // The command line wins over the file's directory, while unrelated
        // file values (the prefix) still apply.
        let (_dir, config) = yaml_config_with_args(
            "elasticsearch:\n  enabled: false\npcap:\n  directory: /from-yaml\n  prefix: yaml.\n",
            &["--pcap-directory", "/from-cli"],
        );
        let channel = channel_from(&config, "https://evebox.test")
            .unwrap()
            .unwrap();
        assert_eq!(channel.spool.directory, PathBuf::from("/from-cli"));
        assert_eq!(channel.spool.prefix.as_deref(), Some("yaml."));
    }

    #[test]
    fn direct_elasticsearch_does_not_start_channel() {
        let (_dir, direct) =
            yaml_config("elasticsearch:\n  enabled: true\npcap:\n  directory: /captures\n");
        assert!(
            channel_from(&direct, "http://evebox.test")
                .unwrap()
                .is_none()
        );

        // A command line directory does not override the direct
        // Elasticsearch incompatibility either.
        let (_dir, direct_cli) = yaml_config_with_args(
            "elasticsearch:\n  enabled: true\n",
            &["--pcap-directory", "/captures"],
        );
        assert!(
            channel_from(&direct_cli, "http://evebox.test")
                .unwrap()
                .is_none()
        );
    }
}
