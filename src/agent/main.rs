// Copyright 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use crate::eve::eve::EveJson;
use crate::eve::filters::AddRuleFilter;
use crate::importer::Importer;
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
    paths: Option<Vec<String>>,
    filename: Option<String>,
    #[serde(rename = "custom-fields")]
    custom_fields: Option<HashMap<String, String>>,
    rules: Option<Vec<String>>,
    #[serde(flatten, rename = "other")]
    other: HashMap<String, config::Value>,
}

fn find_config_filename() -> Option<String> {
    let paths = vec!["./agent.yaml", "/etc/evebox/agent.yaml"];
    for path in paths {
        debug!("Checking for {:?}", path);
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

    if settings.get_or_none::<Option<String>>("config")?.is_none() {
        info!("No configuration file provided, checking default locations");
        if let Some(config_path) = find_config_filename() {
            info!("Using configuration file {:?}", config_path);
            settings.merge_file(&config_path)?;
        }
    }

    let mut input_paths = Vec::new();

    let input: InputConfig = match settings.get("input") {
        Ok(input) => input,
        Err(err) => {
            error!("Failed to load inputs: {}", err);
            std::process::exit(1);
        }
    };

    if let Some(filename) = input.filename {
        input_paths.push(filename);
    }

    if let Some(paths) = input.paths {
        input_paths.extend(paths);
    }

    if input_paths.is_empty() {
        error!("no inputs, aborting");
        std::process::exit(1);
    }

    let server: String = match settings.get("server.url") {
        Ok(server) => server,
        Err(_) => {
            error!("error: no EveBox server specified");
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
        let rulemap = crate::rules::load_rules(rule_files);
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
                warn!("Failed to open GeoIP database: {}", err);
            }
            Ok(geoipdb) => {
                filters.push(crate::eve::filters::EveFilter::GeoIP(geoipdb));
            }
        }
    }

    if let Some(custom_fields) = &input.custom_fields {
        for (field, value) in custom_fields {
            info!("Adding custom field: {} -> {:?}", field, value);
            let filter = crate::eve::filters::CustomFieldFilter {
                field: field.to_string(),
                value: value.to_string(),
            };
            filters.push(crate::eve::filters::EveFilter::CustomFieldFilter(filter));
        }
    }

    let filters = Arc::new(filters);

    info!("Server: {}", server);

    let mut input_filenames = Vec::new();

    for input_path in input_paths {
        for path in crate::path::expand(&input_path)? {
            let path = path.display().to_string();
            if !path.ends_with(".bookmark") {
                debug!("Found input file {}", &path);
                input_filenames.push(path);
            }
        }
    }

    let mut tasks = Vec::new();
    for filename in input_filenames {
        info!("Starting file processor on {}", filename);
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
            info!("Using bookmark file: {:?}", bookmark_filename);
        } else {
            warn!(
                "Failed to determine usable bookmark filename, will start reading at end of file"
            );
            end = true;
        }
        let mut processor = eve::Processor::new(reader, Importer::EveBox(importer));
        processor.end = end;

        let local_filters = vec![
            crate::eve::filters::EveFilter::Filters(filters.clone()),
            crate::eve::filters::EveFilter::EveBoxMetadataFilter(
                crate::eve::filters::EveBoxMetadataFilter {
                    filename: Some(filename.to_string()),
                },
            ),
        ];

        processor.filters = Arc::new(local_filters);
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
        trace!("Committing {} events (bytes: {})", n, size);
        let r = self.client.post("api/1/submit")?.body(body).send().await?;
        let status_code = r.status();
        if status_code != 200 {
            let response_body = r.text().await?;
            if !response_body.is_empty() {
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
