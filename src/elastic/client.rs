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

use serde::Deserialize;
use serde::Serialize;
use std::cmp::Ordering;
use std::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("request: {0}")]
    ReqwestError(reqwest::Error),
    #[error("json: {0}")]
    JsonError(serde_json::error::Error),
    #[error("failed to parse version: {0}")]
    VersionParseError(String),
    #[error("{0}")]
    StringError(String),
}

impl From<reqwest::Error> for ClientError {
    fn from(err: reqwest::Error) -> Self {
        ClientError::ReqwestError(err)
    }
}

impl From<serde_json::error::Error> for ClientError {
    fn from(err: serde_json::error::Error) -> Self {
        ClientError::JsonError(err)
    }
}

#[derive(Debug, Default)]
pub struct Client {
    url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
    pub version: RwLock<Option<Version>>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        let version = self.version.read().unwrap();
        Self {
            url: self.url.clone(),
            disable_certificate_validation: self.disable_certificate_validation,
            username: self.username.clone(),
            password: self.password.clone(),
            version: RwLock::new(version.clone()),
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
            .get(&url)
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
            .post(&url)
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
            .put(&url)
            .header("Content-Type", "application/json");
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }

    #[inline(always)]
    pub async fn get_version(&self) -> Result<Version, ClientError> {
        if let Ok(version) = self.version.read() {
            if let Some(version) = &*version {
                return Ok(version.clone());
            }
        }
        let body = self.get("")?.send().await?.text().await?;
        let response: super::ElasticResponse = serde_json::from_str(&body)?;
        if let Some(error) = response.error {
            return Err(ClientError::StringError(error.reason));
        }
        if response.version.is_none() {
            return Err(ClientError::StringError(
                "request for version did not return a version".to_string(),
            ));
        }
        let version = Version::parse(&response.version.unwrap().number)?;
        let mut locked = self.version.write().unwrap();
        *locked = Some(version.clone());
        Ok(version)
    }

    pub async fn put_template(&self, name: &str, template: String) -> Result<(), ClientError> {
        let path = format!("_template/{}", name);
        let response = self.put(&path)?.body(template).send().await?;
        if response.status().as_u16() == 200 {
            return Ok(());
        }
        let body = response.text().await?;
        return Err(ClientError::StringError(body));
    }

    pub async fn get_template(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        let path = format!("_template/{}", name);
        let response = self.get(&path)?.send().await?;
        if response.status() == reqwest::StatusCode::OK {
            let template: serde_json::Value = response.json().await?;
            return Ok(Some(template));
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        return Err(format!("Failed to get template: {}", response.status()).into());
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
                    .map_err(|_| ClientError::VersionParseError(s.to_string()))?;
            } else if i == 1 {
                minor = part
                    .parse::<u64>()
                    .map_err(|_| ClientError::VersionParseError(s.to_string()))?;
            } else if i == 2 {
                patch = part
                    .parse::<u64>()
                    .map_err(|_| ClientError::VersionParseError(s.to_string()))?;
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
            version: RwLock::new(None),
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
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn has_error(&self) -> bool {
        if let Some(errors) = self.errors {
            return errors;
        }
        if let Some(_) = self.error {
            return true;
        }
        return false;
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
