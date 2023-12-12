// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("json: {0}")]
    SerdeJson(#[from] serde_json::error::Error),
    #[error("failed to parse version: {0}")]
    VersionParse(String),
    #[error("{0}")]
    String(String),
}

#[derive(Debug, Default)]
pub struct Client {
    pub url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            disable_certificate_validation: self.disable_certificate_validation,
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            ..Default::default()
        }
    }

    pub fn get_http_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder();
        if self.disable_certificate_validation {
            builder = builder.danger_accept_invalid_certs(true);
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

    pub async fn get_info(&self) -> Result<InfoResponse, ClientError> {
        Ok(self.get("/")?.send().await?.json().await?)
    }

    pub async fn wait_for_info(&self) -> InfoResponse {
        loop {
            match self.get_info().await {
                Ok(response) => {
                    return response;
                }
                Err(err) => {
                    warn!(
                        "Failed to get Elasticsearch version from {}, will try again: {:?}",
                        self.url, err
                    );
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    pub async fn put_template(&self, name: &str, template: String) -> Result<(), ClientError> {
        let path = format!("_template/{name}");
        let response = self.put(&path)?.body(template).send().await?;
        if response.status().as_u16() == 200 {
            return Ok(());
        }
        let body = response.text().await?;
        Err(ClientError::String(body))
    }

    pub async fn get_template(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        let path = format!("_template/{name}");
        let response = self.get(&path)?.send().await?;
        if response.status() == reqwest::StatusCode::OK {
            let template: serde_json::Value = response.json().await?;
            return Ok(Some(template));
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Err(format!("Failed to get template: {}", response.status()).into())
    }

    pub async fn get_indices(&self) -> anyhow::Result<HashMap<String, Value>> {
        let response = self.get("_all")?.send().await?;
        let response = response.json().await?;
        Ok(response)
    }
}

#[derive(Debug, Clone, Eq)]
pub struct Version {
    pub version: String,
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
            version: s.to_string(),
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
pub struct ClientBuilder {
    url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
}

impl ClientBuilder {
    pub fn new(url: &str) -> ClientBuilder {
        ClientBuilder {
            url: url.to_string(),
            ..ClientBuilder::default()
        }
    }

    pub fn disable_certificate_validation(&mut self, yes: bool) -> &Self {
        self.disable_certificate_validation = yes;
        self
    }

    pub fn with_username(&mut self, username: &str) -> &Self {
        self.username = Some(username.to_string());
        self
    }

    pub fn with_password(&mut self, password: &str) -> &Self {
        self.password = Some(password.to_string());
        self
    }

    pub fn build(&self) -> Client {
        Client {
            url: self.url.clone(),
            disable_certificate_validation: self.disable_certificate_validation,
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BulkResponse {
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
pub struct InfoResponse {
    pub name: String,
    pub cluster_name: String,
    pub cluster_uuid: String,
    pub version: InfoResponseVersion,
    pub tagline: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct InfoResponseVersion {
    pub distribution: Option<String>,
    pub number: String,
    pub minimum_wire_compatibility_version: String,
    pub minimum_index_compatibility_version: String,
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
