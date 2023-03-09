// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::eventrepo::DatastoreError;
pub use client::Version;
pub use client::{Client, ClientBuilder};
pub use eventrepo::ElasticEventRepo;
pub use importer::Importer;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use time::macros::format_description;
use time::OffsetDateTime;

pub mod client;
pub mod eventrepo;
pub mod importer;
pub mod request;
pub mod template_installer;

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
    ReqwestError(#[from] reqwest::Error),
}

impl From<reqwest::Error> for DatastoreError {
    fn from(err: reqwest::Error) -> Self {
        DatastoreError::ElasticSearchError(err.to_string())
    }
}

#[derive(Default, Debug)]
pub struct AlertQueryOptions {
    pub timestamp_gte: Option<OffsetDateTime>,
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

pub fn format_timestamp(dt: time::OffsetDateTime) -> String {
    let format =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
    dt.to_offset(time::UtcOffset::UTC).format(&format).unwrap()
}

pub fn query_string_query(query_string: &str) -> serde_json::Value {
    let escaped = query_string
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('!', "\\!")
        .replace('/', "\\/")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('-', "\\-");
    json!({
        "query_string": {
            "default_operator": "AND",
            "query": escaped,
            "lenient": true,
        }
    })
}

#[derive(Deserialize, Debug)]
pub struct ElasticResponse {
    pub hits: Option<serde_json::Value>,
    pub error: Option<ElasticResponseError>,
    pub status: Option<u64>,
    pub failures: Option<Vec<serde_json::Value>>,
    pub total: Option<u64>,
    pub updated: Option<u64>,
    pub aggregations: Option<serde_json::Value>,
    pub version: Option<response::Version>,

    #[serde(flatten)]
    pub other: std::collections::HashMap<String, serde_json::Value>,
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
    pub other: std::collections::HashMap<String, serde_json::Value>,
}
