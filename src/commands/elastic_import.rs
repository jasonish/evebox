// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::bookmark;
use crate::elastic;
use crate::elastic::template_installer;
use crate::eve;
use crate::eve::Processor;
use crate::importer::Importer;
use crate::logger::log;
use crate::settings::Settings;

pub const DEFAULT_BATCH_SIZE: u64 = 300;
pub const NO_CHECK_CERTIFICATE: &str = "no-check-certificate";

#[derive(Default, Clone, Debug)]
struct ElasticImportConfig {
    end: bool,
    use_bookmark: bool,
    bookmark_filename: PathBuf,
    oneshot: bool,
    stdout: bool,
    disable_geoip: bool,
    geoip_filename: Option<String>,
    batch_size: u64,
}

pub async fn main(args: &clap::ArgMatches<'static>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ElasticImportConfig::default();
    let mut settings = Settings::new(args);

    let elastic_url: String = settings.get("elasticsearch")?;
    let index: String = settings.get("index")?;
    config.end = settings.get_bool("end")?;
    config.use_bookmark = settings.get_bool("bookmark")?;
    config.bookmark_filename = settings.get("bookmark-filename")?;
    config.oneshot = settings.get_bool("oneshot")?;
    config.stdout = settings.get_bool("stdout")?;
    config.disable_geoip = settings.get_bool("geoip.disabled")?;
    config.geoip_filename = settings.get_or_none("geoip.database-filename")?;
    config.batch_size = settings.get("batch-size").unwrap_or(DEFAULT_BATCH_SIZE);
    let bookmark_dir: String = settings.get("bookmark-dir")?;
    let disable_certificate_validation = settings.get_bool(NO_CHECK_CERTIFICATE)?;
    let inputs: Vec<String> = settings.get_string_array("input")?;

    // Bookmark filename and bookmark directory can't be used together.
    if args.occurrences_of("bookmark-filename") > 0 && args.occurrences_of("bookmark-dir") > 0 {
        return Err("--bookmark-filename and --bookmark-dir not allowed together".into());
    }

    if config.use_bookmark {
        let path = PathBuf::from(&bookmark_dir);
        if !path.exists() {
            log::warn!(
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
            log::info!("Bookmark directory created: {}", &path.display());
        }

        // Attempt to write a file into the bookmark directory to make sure its writable
        // by us.
        let tmpfile = path.join(".evebox");
        log::debug!(
            "Testing for bookmark directory writability with file: {}",
            tmpfile.display(),
        );
        match std::fs::File::create(&tmpfile) {
            Ok(file) => {
                log::debug!(directory = ?path, "Bookmark directory is writable:");
                std::mem::drop(file);
                let _ = std::fs::remove_file(&tmpfile);
            }
            Err(err) => {
                log::error!(directory = ?path, "Bookmark directory is not writable: {}:", err);
                std::process::exit(1);
            }
        }
    }

    let username: Option<String> = settings.get_or_none("username")?;
    let password: Option<String> = settings.get_or_none("password")?;

    let mut client = crate::elastic::ClientBuilder::new(&elastic_url);
    client.disable_certificate_validation(disable_certificate_validation);
    if let Some(username) = &username {
        client.with_username(&username);
    }
    if let Some(password) = &password {
        client.with_password(&password);
    }

    let importer = crate::elastic::importer::Importer::new(&index, client.build());

    let mut elastic_client = crate::elastic::ClientBuilder::new(&elastic_url);
    elastic_client.disable_certificate_validation(disable_certificate_validation);
    if let Some(username) = &username {
        elastic_client.with_username(&username);
    }
    if let Some(password) = &password {
        elastic_client.with_password(&password);
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
                log::error!(
                    "Failed to get Elasticsearch version, will try again: error={}",
                    err
                );
                tokio::time::delay_for(Duration::from_secs(1)).await;
            }
        }
    }
    log::info!(
        "Found Elasticsearch version {} at {}",
        version.version,
        &elastic_url
    );
    if version < elastic::Version::parse("7.4.0").unwrap() {
        return Err(format!(
            "Elasticsearch versions less than 7.4.0 not supported (found version {})",
            version.version
        )
        .into());
    }

    if let Err(err) = template_installer::install_template(&elastic_client, &index).await {
        log::error!(
            "Failed to install Elasticsearch template \"{}\": {}",
            &index,
            err
        );
    }

    let is_oneshot = config.oneshot;
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<bool>(); // tokio::sync::oneshot::channel::<bool>();

    for input in &inputs {
        let importer = Importer::Elastic(importer.clone());

        //let importer = importer.clone();
        let input = (*input).to_string();
        let mut config = config.clone();

        if inputs.len() > 1 && config.use_bookmark {
            log::debug!("Getting bookmark filename for {}", &input);
            let bookmark_filename = bookmark::bookmark_filename(&input, &bookmark_dir);
            config.bookmark_filename = bookmark_filename;
            log::debug!(
                "Bookmark filename for {}: {:?}",
                input,
                config.bookmark_filename
            );
        } else {
            // Determine bookmark filename for single file.
            //
            // TODO: If <curdir>.bookmark, convert to <hash>.bookmark.
            let empty_path = PathBuf::from("");
            if bookmark_dir == "." && config.bookmark_filename == empty_path {
                let old_bookmark_filename = std::path::PathBuf::from(".bookmark");
                let new_bookmark_filename = bookmark::bookmark_filename(&input, &bookmark_dir);
                let exists = std::path::Path::exists(&new_bookmark_filename);
                if exists {
                    config.bookmark_filename = new_bookmark_filename;
                } else if Path::exists(&old_bookmark_filename) {
                    config.bookmark_filename = old_bookmark_filename;
                } else {
                    config.bookmark_filename = new_bookmark_filename;
                }
            } else if bookmark_dir != "." {
                let bookmark_filename = bookmark::bookmark_filename(&input, &bookmark_dir);
                config.bookmark_filename = bookmark_filename;
            }
        }

        let done_tx = done_tx.clone();
        let t = tokio::spawn(async move {
            if let Err(err) = import_task(importer, &input, &config).await {
                log::error!("{}: {}", input, err);
            }
            if !config.oneshot {
                done_tx.send(true).expect("Failed to send done signal");
            }
        });

        // If one shot mode, we process each file sequentially.
        if is_oneshot {
            log::info!("In oneshot mode, waiting for task to finish.");
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Starting reader on {}", filename);
    let reader = eve::EveReader::new(filename);
    let bookmark_path = PathBuf::from(&config.bookmark_filename);

    let mut filters = Vec::new();
    if config.disable_geoip {
        log::debug!("GeoIP disabled")
    } else {
        match crate::geoip::GeoIP::open(config.geoip_filename.clone()) {
            Err(err) => {
                log::warn!("Failed to open GeoIP database: {}", err);
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
