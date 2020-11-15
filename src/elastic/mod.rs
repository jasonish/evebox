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

use crate::datastore::DatastoreError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use thiserror::Error;

pub mod client;
pub use client::{Client, ClientBuilder};
pub mod importer;
pub use importer::Importer;
pub mod eventstore;
pub use client::Version;
pub use eventstore::EventStore;
pub mod report;
pub mod request;

pub mod template_installer;
pub const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S.%3fZ";

pub const ACTION_ARCHIVED: &str = "archived";
pub const ACTION_ESCALATED: &str = "escalated";
pub const ACTION_DEESCALATED: &str = "de-escalated";
pub const ACTION_COMMENT: &str = "comment";

pub const TAG_ESCALATED: &str = "evebox.escalated";
pub const TAGS_ESCALATED: [&str; 1] = [TAG_ESCALATED];
pub const TAG_ARCHIVED: &str = "evebox.archived";
pub const TAGS_ARCHIVED: [&str; 1] = [TAG_ARCHIVED];

#[derive(Debug, Error)]
pub enum ElasticError {
    #[error("elasticsearch response error: {0}")]
    ErrorResponse(String),
    #[error("elasticsearch: reqwest error: {0}")]
    ReqwestError(reqwest::Error),
}

impl From<ElasticError> for DatastoreError {
    fn from(err: ElasticError) -> Self {
        DatastoreError::ElasticError(err)
    }
}

impl From<reqwest::Error> for DatastoreError {
    fn from(err: reqwest::Error) -> Self {
        DatastoreError::ElasticSearchError(err.to_string())
    }
}

impl From<reqwest::Error> for ElasticError {
    fn from(err: reqwest::Error) -> Self {
        ElasticError::ReqwestError(err)
    }
}

#[derive(Default)]
pub struct AlertQueryOptions {
    pub timestamp_gte: Option<chrono::DateTime<chrono::Utc>>,
    pub query_string: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct HistoryEntry {
    pub username: String,
    pub timestamp: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

pub fn format_timestamp<Tz: chrono::offset::TimeZone>(dt: chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    dt.format(TIME_FORMAT).to_string()
}

pub fn query_string_query(query_string: &str) -> JsonValue {
    json!({
        "query_string": {
            "default_operator": "AND",
            "query": query_string,
            "lenient": true,
        }
    })
}

#[derive(Deserialize, Debug)]
pub struct ElasticResponse {
    pub hits: Option<serde_json::Value>,
    pub error: Option<ElasticResponseError>,
    pub status: Option<u64>,
    pub failures: Option<Vec<JsonValue>>,
    pub total: Option<u64>,
    pub updated: Option<u64>,
    pub aggregations: Option<serde_json::Value>,
    pub version: Option<response::Version>,

    #[serde(flatten)]
    pub other: std::collections::HashMap<String, JsonValue>,
}

pub mod response {
    use super::Deserialize;
    #[derive(Deserialize, Debug)]
    pub struct Version {
        pub number: String,
    }
}

#[derive(Deserialize, Debug)]
pub struct ElasticResponseError {
    pub reason: String,
    #[serde(flatten)]
    pub other: std::collections::HashMap<String, JsonValue>,
}
