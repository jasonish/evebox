// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

use crate::bookmark;
use crate::elastic;
use crate::elastic::template_installer;
use crate::eve;
use crate::eve::filters::{AddRuleFilter, EveFilter};
use crate::eve::Processor;
use crate::importer::Importer;
use crate::prelude::*;
use clap::parser::ValueSource;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_BATCH_SIZE: u64 = 300;
pub const NO_CHECK_CERTIFICATE: &str = "no-check-certificate";

#[derive(Default, Clone, Debug)]
struct ElasticImportConfig {
    end: bool,
    use_bookmark: bool,
    bookmark_filename: PathBuf,
    oneshot: bool,
    disable_geoip: bool,
    geoip_filename: Option<String>,
    elastic_url: String,
    elastic_username: Option<String>,
    elastic_password: Option<String>,
    index: String,
    no_index_suffix: bool,
    bookmark_dir: String,
    disable_certificate_validation: bool,
}

pub async fn main(args: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let config_filename = args.get_one::<String>("config");
    let loader = crate::config::Config::new(args, config_filename.map(|x| &**x))?;

    let config = ElasticImportConfig {
        elastic_url: loader.get_string("elasticsearch").unwrap(),
        elastic_username: loader.get_string("username"),
        elastic_password: loader.get_string("password"),
        index: loader.get_string("index").unwrap(),
        no_index_suffix: loader.get_bool("no-index-suffix")?,
        end: loader.get_bool("end")?,
        use_bookmark: loader.get_bool("bookmark")?,
        bookmark_filename: loader.get_string("bookmark-filename").unwrap().into(),
        oneshot: loader.get_bool("oneshot")?,
        disable_geoip: loader.get_bool("geoip.disabled")?,
        geoip_filename: loader.get_string("geoip.database-filename"),
        bookmark_dir: loader.get_string("bookmark-dir").unwrap(),
        disable_certificate_validation: loader.get_bool(NO_CHECK_CERTIFICATE)?,
    };

    let inputs = match loader.get_arg_strings("input") {
        Some(inputs) => inputs,
        None => match loader.get_config_value("input")? {
            Some(inputs) => inputs,
            None => {
                fatal!("no input files provided");
            }
        },
    };

    // Bookmark filename and bookmark directory can't be used together.
    if args.value_source("bookmark-filename") == Some(ValueSource::CommandLine)
        && args.value_source("bookmark-dir") == Some(ValueSource::CommandLine)
    {
        return Err("--bookmark-filename and --bookmark-dir not allowed together".into());
    }

    // If multiple inputs are used, --bookmark-filename cannot be used.
    if inputs.len() > 1 && args.value_source("bookmark-filename") == Some(ValueSource::CommandLine)
    {
        return Err("--bookmark-filename cannot be used with multiple inputs".into());
    }

    if config.use_bookmark {
        let path = PathBuf::from(&config.bookmark_dir);
        if !path.exists() {
            warn!(
                "Bookmark directory does not exist: {}",
                &path.to_str().unwrap()
            );
            std::fs::create_dir_all(&path).map_err(|err| {
                format!(
                    "Failed to create bookmark directory: {}: {}",
                    &path.display(),
                    err
                )
            })?;
            info!("Bookmark directory created: {}", &path.display());
        }

        // Attempt to write a file into the bookmark directory to make sure its writable
        // by us.
        let tmpfile = path.join(".evebox");
        debug!(
            "Testing for bookmark directory writability with file: {}",
            tmpfile.display(),
        );
        match std::fs::File::create(&tmpfile) {
            Ok(file) => {
                debug!(directory = ?path, "Bookmark directory is writable:");
                std::mem::drop(file);
                let _ = std::fs::remove_file(&tmpfile);
            }
            Err(err) => {
                error!(directory = ?path, "Bookmark directory is not writable: {}:", err);
                std::process::exit(1);
            }
        }
    }

    let mut client = crate::elastic::ClientBuilder::new(&config.elastic_url);
    client.disable_certificate_validation(config.disable_certificate_validation);
    if let Some(username) = &config.elastic_username {
        client.with_username(username);
    }
    if let Some(password) = &config.elastic_password {
        client.with_password(password);
    }

    debug!(
        "Elasticsearch index: {}, no-index-suffix={}",
        &config.index, config.no_index_suffix
    );
    let importer = crate::elastic::importer::Importer::new(
        client.build(),
        &config.index,
        config.no_index_suffix,
    );

    let mut elastic_client = crate::elastic::ClientBuilder::new(&config.elastic_url);
    elastic_client.disable_certificate_validation(config.disable_certificate_validation);
    if let Some(username) = &config.elastic_username {
        elastic_client.with_username(username);
    }
    if let Some(password) = &config.elastic_password {
        elastic_client.with_password(password);
    }
    let elastic_client = elastic_client.build();

    let version;
    loop {
        match elastic_client.get_version().await {
            Ok(v) => {
                version = v;
                break;
            }
            Err(err) => {
                error!(
                    "Failed to get Elasticsearch version, will try again: error={}",
                    err
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    info!(
        "Found Elasticsearch version {} at {}",
        version.version, &config.elastic_url
    );
    if version < elastic::Version::parse("7.4.0").unwrap() {
        return Err(format!(
            "Elasticsearch versions less than 7.4.0 not supported (found version {})",
            version.version
        )
        .into());
    }

    if let Err(err) = template_installer::install_template(&elastic_client, &config.index).await {
        error!(
            "Failed to install Elasticsearch template \"{}\": {}",
            &config.index, err
        );
    }

    let mut filters = Vec::new();

    match loader.get_strings("rules") {
        Ok(Some(rules)) => {
            if !rules.is_empty() {
                let rulemap = crate::rules::load_rules(&rules);
                let rulemap = Arc::new(rulemap);
                filters.push(crate::eve::filters::EveFilter::AddRuleFilter(
                    AddRuleFilter {
                        map: rulemap.clone(),
                    },
                ));
                crate::rules::watch_rules(rulemap);
            }
        }
        Ok(None) => {}
        Err(err) => {
            error!("Failed to read input.rules configuration: {}", err);
        }
    }

    let filters = Arc::new(filters);

    let is_oneshot = config.oneshot;
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<bool>(); // tokio::sync::oneshot::channel::<bool>();

    for input in &inputs {
        let importer = Importer::Elastic(importer.clone());

        //let importer = importer.clone();
        let input = (*input).to_string();
        let mut config = config.clone();

        if inputs.len() > 1 && config.use_bookmark {
            debug!("Getting bookmark filename for {}", &input);
            let bookmark_filename = bookmark::bookmark_filename(&input, &config.bookmark_dir);
            config.bookmark_filename = bookmark_filename;
            debug!(
                "Bookmark filename for {}: {:?}",
                input, config.bookmark_filename
            );
        } else {
            // Determine bookmark filename for single file.
            //
            // TODO: If <curdir>.bookmark, convert to <hash>.bookmark.
            let empty_path = PathBuf::from("");
            if config.bookmark_dir == "." && config.bookmark_filename == empty_path {
                let old_bookmark_filename = std::path::PathBuf::from(".bookmark");
                let new_bookmark_filename =
                    bookmark::bookmark_filename(&input, &config.bookmark_dir);
                let exists = std::path::Path::exists(&new_bookmark_filename);
                if exists {
                    config.bookmark_filename = new_bookmark_filename;
                } else if Path::exists(&old_bookmark_filename) {
                    config.bookmark_filename = old_bookmark_filename;
                } else {
                    config.bookmark_filename = new_bookmark_filename;
                }
            } else if config.bookmark_dir != "." {
                let bookmark_filename = bookmark::bookmark_filename(&input, &config.bookmark_dir);
                config.bookmark_filename = bookmark_filename;
            }
        }

        let done_tx = done_tx.clone();
        let filters = filters.clone();
        let t = tokio::spawn(async move {
            if let Err(err) = import_task(importer, &input, &config, filters).await {
                error!("{}: {}", input, err);
            }
            if !config.oneshot {
                done_tx.send(true).expect("Failed to send done signal");
            }
        });

        // If one shot mode, we process each file sequentially.
        if is_oneshot {
            info!("In oneshot mode, waiting for task to finish.");
            t.await.unwrap();
        }
    }

    if !config.oneshot {
        done_rx.recv().await;
    }

    Ok(())
}

async fn import_task(
    importer: Importer,
    filename: &str,
    config: &ElasticImportConfig,
    root_filters: Arc<Vec<EveFilter>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting reader on {}", filename);
    let reader = eve::EveReader::new(filename);
    let bookmark_path = PathBuf::from(&config.bookmark_filename);

    let mut filters = vec![EveFilter::Filters(root_filters)];
    if config.disable_geoip {
        debug!("GeoIP disabled");
    } else {
        match crate::geoip::GeoIP::open(config.geoip_filename.clone()) {
            Err(err) => {
                warn!("Failed to open GeoIP database: {}", err);
            }
            Ok(geoipdb) => {
                filters.push(crate::eve::filters::EveFilter::GeoIP(geoipdb));
            }
        }
    }

    filters.push(crate::eve::filters::EveFilter::EveBoxMetadataFilter(
        crate::eve::filters::EveBoxMetadataFilter {
            filename: Some(filename.to_string()),
        },
    ));

    let filters = Arc::new(filters);

    let mut processor = Processor::new(reader, importer);
    if config.use_bookmark {
        processor.bookmark_filename = Some(bookmark_path.clone());
    }
    processor.end = config.end;
    processor.filters = filters;
    processor.report_interval = Duration::from_secs(60);
    processor.oneshot = config.oneshot;

    processor.run().await;
    Ok(())
}
