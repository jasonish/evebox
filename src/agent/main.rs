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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use crate::eve::eve::EveJson;
use crate::eve::filters::AddRuleFilter;
use crate::importer::Importer;
use crate::logger::log;
use crate::settings::Settings;
use crate::{bookmark, eve};

#[derive(Debug, Deserialize)]
struct ServerConfig {
    url: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InputConfig {
    filename: String,
    #[serde(rename = "custom-fields")]
    custom_fields: Option<HashMap<String, String>>,
    rules: Option<Vec<String>>,
    #[serde(flatten, rename = "other")]
    other: HashMap<String, config::Value>,
}

fn find_config_filename() -> Option<String> {
    let paths = vec!["./agent.yaml", "/etc/evebox/agent.yaml"];
    for path in paths {
        log::debug!("Checking for {:?}", path);
        let path_buf = PathBuf::from(path);
        if path_buf.exists() {
            return Some(path.to_string());
        }
    }
    None
}

pub async fn main(args: &clap::ArgMatches<'static>) -> anyhow::Result<()> {
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        std::process::exit(0);
    });

    let mut settings = Settings::new(args);

    if let None = settings.get_or_none::<Option<String>>("config")? {
        log::info!("No configuration file provided, checking default locations");
        if let Some(config_path) = find_config_filename() {
            log::info!("Using configuration file {:?}", config_path);
            settings.merge_file(&config_path)?;
        }
    }

    let input: InputConfig = match settings.get("input") {
        Ok(input) => input,
        Err(err) => {
            log::error!("Failed to load inputs: {}", err);
            std::process::exit(1);
        }
    };

    let server: String = match settings.get("server.url") {
        Ok(server) => server,
        Err(_) => {
            log::error!("error: no EveBox server specified");
            std::process::exit(1);
        }
    };
    let username: Option<String> = settings.get_or_none("server.username").unwrap();
    let password: Option<String> = settings.get_or_none("server.password").unwrap();
    let disable_certificate_validation = settings
        .get_bool("disable-certificate-check")
        .unwrap_or(false);
    let bookmark_directory: Option<String> = settings.get_or_none("bookmark-directory").unwrap();
    let data_directory: Option<String> = settings.get_or_none("data-directory").unwrap();
    let enable_geoip = settings.get_bool("geoip.enabled").unwrap();

    let mut filters = Vec::new();

    if let Some(rule_files) = &input.rules {
        let rulemap = crate::rules::load_rules(&rule_files);
        let rulemap = Arc::new(rulemap);
        filters.push(crate::eve::filters::EveFilter::AddRuleFilter(
            AddRuleFilter {
                map: rulemap.clone(),
            },
        ));
        crate::rules::watch_rules(rulemap);
    }

    if enable_geoip {
        match crate::geoip::GeoIP::open(None) {
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
            filename: Some(input.filename.to_string()),
        },
    ));

    if let Some(custom_fields) = &input.custom_fields {
        for (field, value) in custom_fields {
            log::info!("Adding custom field: {} -> {:?}", field, value);
            let filter = crate::eve::filters::CustomFieldFilter {
                field: field.to_string(),
                value: value.to_string(),
            };
            filters.push(crate::eve::filters::EveFilter::CustomFieldFilter(filter));
        }
    }

    let filters = Arc::new(filters);

    log::info!("Server: {}", server);

    let mut filenames = Vec::new();
    match glob::glob(&input.filename) {
        Err(_) => {
            log::error!(
                "The provided input filename is an invalid pattern: {}",
                &input.filename
            );
            std::process::exit(1);
        }
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(path) => {
                        let path = path.display().to_string();
                        if !path.ends_with(".bookmark") {
                            log::debug!("Found input file {}", &path);
                            filenames.push(path);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
    if filenames.is_empty() {
        filenames.push(input.filename.clone());
    }

    let mut tasks = Vec::new();
    for filename in filenames {
        log::info!("Starting file processor on {}", filename);
        let mut end = false;
        let reader = crate::eve::reader::EveReader::new(&filename);
        let client = Client::new(
            &server,
            username.clone(),
            password.clone(),
            disable_certificate_validation,
        );
        let importer = EveboxImporter::new(client.clone());
        let bookmark_filename = get_bookmark_filename(
            &filename,
            if bookmark_directory.is_some() {
                bookmark_directory.clone()
            } else {
                data_directory.clone()
            },
        );
        if let Some(bookmark_filename) = &bookmark_filename {
            log::info!("Using bookmark file: {:?}", bookmark_filename);
        } else {
            log::warn!(
                "Failed to determine usable bookmark filename, will start reading at end of file"
            );
            end = true;
        }
        let mut processor = eve::Processor::new(reader, Importer::EveBox(importer));
        processor.end = end;
        processor.filters = filters.clone();
        processor.report_interval = Duration::from_secs(60);
        processor.bookmark_filename = bookmark_filename;
        let t = tokio::spawn(async move {
            processor.run().await;
        });
        tasks.push(t);
    }

    for task in tasks {
        task.await.unwrap();
    }

    Ok(())
}

fn get_bookmark_filename(input: &str, directory: Option<String>) -> Option<PathBuf> {
    if let Some(directory) = directory {
        return Some(bookmark::bookmark_filename(input, &directory));
    } else {
        let filename = PathBuf::from(format!("{}.bookmark", input));

        if filename.exists() {
            log::info!(
                "Legacy bookmark filename exists, will check if writable: {:?}",
                &filename
            );
            if let Err(err) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&filename)
            {
                log::warn!(
                    "Failed open deprecated bookmark file {:?}, will not use: {}",
                    &filename,
                    err
                );
            } else {
                log::info!("Using deprecated bookmark file {:?}", &filename);
                return Some(filename);
            }
        }

        let filename = bookmark::bookmark_filename(input, ".");
        log::info!("Testing bookmark filename {:?}", filename);
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
                log::info!("Bookmark file {:?} looks OK", filename);
                return Some(filename);
            }
            Err(err) => {
                log::warn!("Error using {:?} as bookmark filename: {}", filename, err);
            }
        }
    }
    return None;
}

// EveBox agent import. For importing events to an EveBox server.
#[derive(Debug, Clone)]
pub struct EveboxImporter {
    pub client: Client,
    pub queue: Vec<String>,
}

impl EveboxImporter {
    pub fn new(client: Client) -> Self {
        Self {
            queue: Vec::new(),
            client: client,
        }
    }

    pub async fn submit(
        &mut self,
        event: EveJson,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.queue.push(event.to_string());
        Ok(())
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        let n = self.queue.len();
        let body = self.queue.join("\n");
        let size = body.len();
        log::trace!("Committing {} events (bytes: {})", n, size);
        let r = self.client.post("api/1/submit")?.body(body).send().await?;
        let status_code = r.status();
        if status_code != 200 {
            let response_body = r.text().await?;
            if response_body != "" {
                if let Ok(error) = serde_json::from_str::<serde_json::Value>(&response_body) {
                    if let serde_json::Value::String(error) = &error["error"] {
                        return Err(anyhow!("{}", error));
                    }
                }
                return Err(anyhow!("{}", response_body));
            }
            return Err(anyhow!("Server returned status code {}", status_code));
        }
        self.queue.truncate(0);
        Ok(n)
    }
}

// EveBox agent client (to EveBox server)
#[derive(Clone, Debug)]
pub struct Client {
    url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
}

impl Client {
    pub fn new(
        url: &str,
        username: Option<String>,
        password: Option<String>,
        disable_certificate_validation: bool,
    ) -> Self {
        Self {
            url: url.to_string(),
            disable_certificate_validation: disable_certificate_validation,
            username: username,
            password: password,
        }
    }

    pub fn get_http_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder();
        if self.disable_certificate_validation {
            builder = builder.danger_accept_invalid_certs(true);
        }
        builder.build()
    }

    pub fn post(&self, path: &str) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let request = self
            .get_http_client()?
            .post(&url)
            .header("Content-Type", "application/json");
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }
}
