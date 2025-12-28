// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::Read;
use std::time::Duration;

use super::ElasticResponseError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ClientError {
    #[error("request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("json: {0}")]
    SerdeJson(#[from] serde_json::error::Error),
    #[error("failed to parse version: {0}")]
    VersionParse(String),
    #[error("{0}")]
    String(String),
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Client {
    url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
    cert: Option<reqwest::Certificate>,
}

fn default_username() -> Option<String> {
    std::env::var("EVEBOX_ELASTICSEARCH_USERNAME").ok()
}

fn default_password() -> Option<String> {
    std::env::var("EVEBOX_ELASTICSEARCH_PASSWORD").ok()
}

fn load_certificate_from_file(filename: &str) -> anyhow::Result<reqwest::Certificate> {
    let file = std::fs::File::open(filename)?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![];
    reader.read_to_end(&mut buffer)?;
    Ok(reqwest::Certificate::from_pem(&buffer)?)
}

/// Load a certificate from the specified in
/// EVEBOX_ELASTICSEARCH_HTTP_CA_CERT. Returning None if not set, or
/// if an error occurs. But log the error before returning None.
fn load_certificate_from_env() -> Option<reqwest::Certificate> {
    if let Ok(filename) = std::env::var("EVEBOX_ELASTICSEARCH_CACERT") {
        match load_certificate_from_file(&filename) {
            Ok(cert) => Some(cert),
            Err(err) => {
                warn!(
                    "Failed to load Elasticsearch HTTP CA certificate from {}, will continue without: {}",
                    filename, err
                );
                None
            }
        }
    } else {
        None
    }
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            username: std::env::var("EVEBOX_ELASTICSEARCH_USERNAME").ok(),
            password: std::env::var("EVEBOX_ELASTICSEARCH_PASSWORD").ok(),
            cert: load_certificate_from_env(),
            ..Default::default()
        }
    }

    pub fn get_http_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder();
        if self.disable_certificate_validation {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if let Some(cert) = self.cert.clone() {
            builder = builder.add_root_certificate(cert);
        }

        builder.build()
    }

    pub fn get(&self, path: &str) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let request = self
            .get_http_client()?
            .get(url)
            .header("Content-Type", "application/json");
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }

    pub fn post(&self, path: &str) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let request = self
            .get_http_client()?
            .post(url)
            .header("Content-Type", "application/json");
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }

    pub fn put(&self, path: &str) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let request = self
            .get_http_client()?
            .put(url)
            .header("Content-Type", "application/json");
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }

    pub fn delete(&self, path: &str) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let request = self
            .get_http_client()?
            .delete(url)
            .header("Content-Type", "application/json");
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }

    /// Put request with a body that can be serialized into JSON.
    pub fn put_json<T: Serialize>(
        &self,
        path: &str,
        body: T,
    ) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let request = self
            .get_http_client()?
            .put(url)
            .header("Content-Type", "application/json")
            .json(&body);
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }

    pub async fn get_info(&self) -> Result<InfoResponse, ClientError> {
        let response = self.get("/")?.send().await?;
        let code = response.status();
        let text = response.text().await?;
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            if json["error"].is_object() {
                let error: ElasticResponseError = serde_json::from_value(json["error"].clone())?;
                Err(ClientError::String(error.first_reason()))
            } else {
                Ok(serde_json::from_value(json)?)
            }
        } else {
            // OpenSearch does not return a JSON error for at least authentication errors.
            let err = format!("{code}");
            Err(ClientError::String(err))
        }
    }

    pub async fn wait_for_info(&self) -> InfoResponse {
        loop {
            match self.get_info().await {
                Ok(response) => {
                    return response;
                }
                Err(err) => {
                    warn!(
                        "Failed to get Elasticsearch version from {}, will try again: {}",
                        self.url, err
                    );
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    pub async fn get_indices(&self) -> anyhow::Result<HashMap<String, Value>> {
        let response = self.get("_all")?.send().await?;
        let response = response.json().await?;
        Ok(response)
    }

    pub(crate) async fn get_indices_pattern(
        &self,
        pattern: &str,
    ) -> anyhow::Result<Vec<IndicesPatternResponse>> {
        let response = self
            .get(&format!("_cat/indices/{pattern}?format=json"))?
            .send()
            .await?;
        let text = response.text().await?;
        let response = serde_json::from_str(&text)?;
        Ok(response)
    }

    pub(crate) async fn get_index_settings(
        &self,
        index: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let response = self.get(&format!("{index}/_settings"))?.send().await?;
        let text = response.text().await?;
        let response = serde_json::from_str(&text)?;
        Ok(response)
    }

    pub(crate) async fn get_template(&self, name: &str) -> anyhow::Result<serde_json::Value> {
        let response = self.get(&format!("_template/{name}"))?.send().await?;
        let response = response.error_for_status()?;
        let text = response.text().await?;
        let mut response: serde_json::Value = serde_json::from_str(&text)?;
        Ok(response[name].take())
    }

    pub(crate) async fn get_index_stats(&self, base: &str) -> Result<Vec<SimpleIndexStats>> {
        let path = format!("{base}*/_stats");
        let response = self.get(&path)?.send().await?;
        let json: serde_json::Value = response.json().await?;
        let indices: HashMap<String, IndexStatsResponse> =
            serde_json::from_value(json["indices"].clone())?;
        let mut keys: Vec<&String> = indices.keys().collect();
        keys.sort();
        let mut simple = vec![];
        for key in keys {
            simple.push(SimpleIndexStats {
                name: key.clone(),
                doc_count: indices[key].primaries.docs.count,
                store_size: indices[key].primaries.store.size_in_bytes,
            });
        }

        Ok(simple)
    }

    pub(crate) async fn delete_index(&self, index: &str) -> anyhow::Result<reqwest::StatusCode> {
        let response = self.delete(index)?.send().await?;
        Ok(response.status())
    }

    pub fn set_username(&mut self, username: Option<String>) {
        self.username = username;
    }

    pub fn set_password(&mut self, password: Option<String>) {
        self.password = password;
    }

    pub fn set_disable_certificate_validation(&mut self, disable: bool) {
        self.disable_certificate_validation = disable;
    }

    pub fn get_url(&self) -> &str {
        &self.url
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct IndicesPatternResponse {
    pub index: String,
}

#[derive(Debug, Clone, Eq)]
pub(crate) struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub fn parse(s: &str) -> Result<Version, ClientError> {
        let mut major = 0;
        let mut minor = 0;
        let mut patch = 0;
        for (i, part) in s.split('.').enumerate() {
            if i == 0 {
                major = part
                    .parse::<u64>()
                    .map_err(|_| ClientError::VersionParse(s.to_string()))?;
            } else if i == 1 {
                minor = part
                    .parse::<u64>()
                    .map_err(|_| ClientError::VersionParse(s.to_string()))?;
            } else if i == 2 {
                patch = part
                    .parse::<u64>()
                    .map_err(|_| ClientError::VersionParse(s.to_string()))?;
            }
        }
        let version = Version {
            major,
            minor,
            patch,
        };
        Ok(version)
    }

    pub fn as_u64(&self) -> u64 {
        (self.major * 1_000_000_000) + (self.minor * 1_000_000) + (self.patch * 1_000)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_u64().cmp(&other.as_u64())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.as_u64() == other.as_u64()
    }
}

#[derive(Default, Debug)]
pub(crate) struct ClientBuilder {
    url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
    cert: Option<reqwest::Certificate>,
}

impl ClientBuilder {
    pub fn new(url: &str) -> ClientBuilder {
        ClientBuilder {
            url: url.to_string(),
            username: default_username(),
            password: default_password(),
            cert: load_certificate_from_env(),
            ..ClientBuilder::default()
        }
    }

    pub fn disable_certificate_validation(mut self, yes: bool) -> Self {
        self.disable_certificate_validation = yes;
        self
    }

    pub fn with_username(mut self, username: &str) -> Self {
        self.username = Some(username.to_string());
        self
    }

    pub fn with_password(mut self, password: &str) -> Self {
        self.password = Some(password.to_string());
        self
    }

    pub(crate) fn with_cacert(mut self, cacert: &str) -> anyhow::Result<Self> {
        self.cert = Some(load_certificate_from_file(cacert)?);
        Ok(self)
    }

    pub fn build(self) -> Client {
        Client {
            url: self.url.clone(),
            disable_certificate_validation: self.disable_certificate_validation,
            username: self.username,
            password: self.password,
            cert: self.cert,
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct BulkResponse {
    pub errors: Option<bool>,
    pub items: Option<Vec<serde_json::Value>>,
    pub error: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: std::collections::HashMap<String, serde_json::Value>,
}

impl BulkResponse {
    pub fn has_error(&self) -> bool {
        if let Some(errors) = self.errors {
            return errors;
        }
        if self.error.is_some() {
            return true;
        }
        false
    }

    pub fn first_error(&self) -> Option<String> {
        if !self.has_error() {
            return None;
        }
        if let Some(error) = &self.error {
            return Some(error.to_string());
        }
        if let Some(items) = &self.items {
            for item in items {
                if let serde_json::Value::String(err) = &item["index"]["error"]["reason"] {
                    return Some(err.to_string());
                }
            }
        }
        None
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct InfoResponse {
    pub version: InfoResponseVersion,
    pub tagline: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct InfoResponseVersion {
    pub distribution: Option<String>,
    pub number: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IndexStatsResponse {
    primaries: IndexStatsPrimaries,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IndexStatsPrimaries {
    pub docs: IndexStatsDocs,
    pub store: IndexStatsStore,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IndexStatsDocs {
    pub count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IndexStatsStore {
    pub size_in_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SimpleIndexStats {
    pub name: String,
    pub doc_count: u64,
    pub store_size: u64,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_version_compare() {
        assert_eq!(
            Version::parse("1.1.1").unwrap(),
            Version::parse("1.1.1").unwrap()
        );
        assert!(Version::parse("7.7.0").unwrap() < Version::parse("7.7.1").unwrap());
        assert!(Version::parse("7.7.1").unwrap() <= Version::parse("7.7.1").unwrap());
        assert!(Version::parse("7.7.1").unwrap() == Version::parse("7.7.1").unwrap());
    }
}
