// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

use crate::agent::client::Client;
use crate::agent::importer::EveboxImporter;
use crate::bookmark;
use crate::config::Config;
use crate::eve::filters::{AddRuleFilter, EveFilter};
use crate::importer::Importer;
use clap::{Arg, Command};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Command line parser for the `agent` sub-command.
pub fn command() -> clap::Command<'static> {
    Command::new("agent")
        .about("EveBox Agent")
        .arg(
            Arg::new("config")
                .long("config")
                .short('c')
                .takes_value(true)
                .help("Agent configuration file"),
        )
        .arg(
            Arg::new("server.url")
                .long("server")
                .takes_value(true)
                .value_name("URL")
                .help("EveBox server URL")
                .env("EVEBOX_SERVER_URL"),
        )
        .arg(
            Arg::new("geoip.enabled")
                .long("enable-geoip")
                .help("Enable MaxMind GeoIP"),
        )
        .arg(
            Arg::new("stdout")
                .long("stdout")
                .help("Print events to stdout"),
        )
        .arg(
            Arg::new("bookmark-directory")
                .long("bookmark-directory")
                .takes_value(true)
                .hide(true),
        )
        .arg(
            Arg::new("filenames")
                .multiple_values(true)
                .help("List of EVE filenames"),
        )
}

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        std::process::exit(0);
    });

    let config_filename = match args.value_of("config") {
        Some(v) => Some(v),
        None => find_config_filename(),
    };
    if let Some(filename) = config_filename {
        debug!("Using configuration file {}", filename);
    }
    let config = Config::new(args, config_filename)?;

    let server_url = config
        .get_string("server.url")
        .unwrap_or_else(|| "http://localhost:5636".to_string());
    let server_username = config.get_string("server.username");
    let server_password = config.get_string("server.password");

    let disable_certificate_check: bool = config
        .get_value("disable-certificate-check")
        .unwrap_or(Some(false))
        .unwrap_or(false);

    // Collect eve filenames.
    let eve_filenames = get_eve_filenames(&config)?;
    if eve_filenames.is_empty() {
        bail!("No EVE log files provided. Exiting as there is nothing to do.");
    }

    let enable_geoip = args.occurrences_of("geoip.enabled") > 0;

    // Get additional fields to add to events.
    let additional_fields = get_additional_fields(&config)?;

    let rule_filenames = get_rule_filenames(&config)?;

    let mut filters: Vec<EveFilter> = vec![];

    if enable_geoip {
        match crate::geoip::GeoIP::open(None) {
            Err(err) => {
                warn!("Failed to open GeoIP database: {}", err);
            }
            Ok(geoipdb) => {
                filters.push(crate::eve::filters::EveFilter::GeoIP(geoipdb));
            }
        }
    }

    if !rule_filenames.is_empty() {
        let rule_collection = Arc::new(crate::rules::load_rules(&rule_filenames));
        filters.push(crate::eve::filters::EveFilter::AddRuleFilter(
            AddRuleFilter {
                map: rule_collection.clone(),
            },
        ));
        crate::rules::watch_rules(rule_collection);
    }

    if let Some(custom_fields) = additional_fields {
        for (field, value) in custom_fields {
            info!("Adding custom field: {} -> {:?}", field, value);
            let filter = crate::eve::filters::CustomFieldFilter {
                field: field.to_string(),
                value: value.to_string(),
            };
            filters.push(crate::eve::filters::EveFilter::CustomFieldFilter(filter));
        }
    }

    let mut log_runners: HashMap<String, bool> = HashMap::new();

    let client = Client::new(
        &server_url,
        server_username.clone(),
        server_password.clone(),
        disable_certificate_check,
    );

    let bookmark_directory = config.get_string("bookmark-directory");
    if bookmark_directory.is_some() {
        warn!("Found deprecated option bookmark-directory, please use data-directory");
    }
    let data_directory = config.get_string("data-directory");

    let bookmark_directory = if bookmark_directory.is_some() {
        bookmark_directory
    } else {
        data_directory
    };

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
                        client.clone(),
                        bookmark_directory.clone(),
                        filters.clone(),
                    );
                    tasks.push(task);
                }
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            _ = tasks.select_next_some() => {
                bail!("A log processing task unexpectedly aborted");
            }
        }
    }
}

fn start_runner(
    filename: &str,
    client: Client,
    bookmark_directory: Option<String>,
    mut filters: Vec<EveFilter>,
) -> JoinHandle<()> {
    let mut end = false;
    let reader = crate::eve::reader::EveReader::new(filename);
    let importer = EveboxImporter::new(client);
    let bookmark_filename = get_bookmark_filename(filename, bookmark_directory);
    if let Some(bookmark_filename) = &bookmark_filename {
        info!("Using bookmark file: {:?}", bookmark_filename);
    } else {
        warn!("Failed to determine usable bookmark filename, will start reading at end of file");
        end = true;
    }
    let mut processor = crate::eve::Processor::new(reader, Importer::EveBox(importer));
    processor.end = end;

    filters.push(crate::eve::filters::EveFilter::EveBoxMetadataFilter(
        crate::eve::filters::EveBoxMetadataFilter {
            filename: Some(filename.to_string()),
        },
    ));

    processor.filters = Arc::new(filters);
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

fn get_additional_fields(config: &Config) -> anyhow::Result<Option<HashMap<String, String>>> {
    match config.get_value::<HashMap<String, String>>("additional-fields") {
        Ok(Some(fields)) => {
            return Ok(Some(fields));
        }
        Ok(None) => {
            // No `additional-fields` found, check `input.custom-fields`.
            match config.get_value::<HashMap<String, String>>("input.custom-fields") {
                Ok(Some(fields)) => {
                    warn!("Found additional fields in deprecated configuration section 'input.custom-fields'");
                    Ok(Some(fields))
                }
                Ok(None) => Ok(None),
                Err(_) => {
                    bail!("There was an error reading 'input.custom-fields' from the configuration file");
                }
            }
        }
        Err(_) => {
            bail!("There was an error reading 'additional-fields' from the configuration file");
        }
    }
}

fn get_eve_filenames(config: &Config) -> anyhow::Result<Vec<String>> {
    let mut eve_filenames: Vec<String> = vec![];

    if config.args.is_present("filenames") {
        eve_filenames.extend(
            config
                .args
                .values_of("filenames")
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

pub fn get_bookmark_filename(input: &str, directory: Option<String>) -> Option<PathBuf> {
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
                if let Ok(meta) = std::fs::metadata(&filename) {
                    if meta.len() == 0 {
                        let _ = std::fs::remove_file(&filename);
                    }
                }
                info!("Bookmark file {:?} looks OK", filename);
                return Some(filename);
            }
            Err(err) => {
                warn!("Error using {:?} as bookmark filename: {}", filename, err);
            }
        }
    }
    return None;
}
