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
use crate::logger::log;
use crate::server::api;
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

pub fn build_inbox_query(options: AlertQueryOptions) -> serde_json::Value {
    let mut filters = Vec::new();
    filters.push(json!({"exists": {"field": "event_type"}}));
    filters.push(json!({"term": {"event_type": "alert"}}));
    if let Some(timestamp_gte) = options.timestamp_gte {
        filters.push(json!({"range": {"@timestamp": {"gte": format_timestamp(timestamp_gte)}}}));
    }
    if let Some(query_string) = options.query_string {
        if !query_string.is_empty() {
            log::info!("Setting query string to: {}", query_string);
            filters.push(query_string_query(&query_string));
        }
    }

    let mut must_not = Vec::new();
    for tag in options.tags {
        if tag.starts_with('-') {
            if tag == "-archived" {
                log::debug!("Rewriting tag {} to {}", tag, "evebox.archived");
                must_not.push(json!({"term": {"tags": "evebox.archived"}}));
            } else {
                let j = json!({"term": {"tags": tag[1..]}});
                must_not.push(j);
            }
        } else if tag == "escalated" {
            log::debug!("Rewriting tag {} to {}", tag, "evebox.escalated");
            let j = json!({"term": {"tags": "evebox.escalated"}});
            filters.push(j);
        } else {
            let j = json!({"term": {"tags": tag}});
            filters.push(j);
        }
    }

    let query = json!({
        "query": {
            "bool": {
                "filter": filters,
                "must_not": must_not,
            }
        },
        "sort": [{"@timestamp": {"order": "desc"}}],
        "aggs": {
            "signatures": {
                "terms": {"field": "alert.signature_id", "size": 10000},
                "aggs": {
                    "sources": {
                        "terms": {"field": "src_ip.keyword", "size": 10000},
                        "aggs": {
                            "destinations": {
                                "terms": {"field": "dest_ip.keyword", "size": 10000},
                                "aggs": {
                                    "escalated": {"filter": {"term": {"tags": "evebox.escalated"}}},
                                    "newest": {"top_hits": {"size": 1, "sort": [{"@timestamp": {"order": "desc"}}]}},
                                    "oldest": {"top_hits": {"size": 1, "sort": [{"@timestamp": {"order": "asc"}}]}}
                                },
                            },
                        },
                    },
                },
            }
        }
    });

    query
}

fn build_alert_group_filter(request: &api::AlertGroupSpec) -> Vec<JsonValue> {
    let mut filter = Vec::new();
    filter.push(json!({"exists": {"field": "event_type"}}));
    filter.push(json!({"term": {"event_type.keyword": "alert"}}));
    filter.push(json!({
        "range": {
            "@timestamp": {
                "gte": request.min_timestamp,
                "lte": request.max_timestamp,
            }
        }
    }));
    filter.push(json!({"term": {"src_ip.keyword": request.src_ip}}));
    filter.push(json!({"term": {"dest_ip.keyword": request.dest_ip}}));
    filter.push(json!({"term": {"alert.signature_id": request.signature_id}}));
    return filter;
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

pub fn timestamp_gte_query<Tz: chrono::offset::TimeZone>(dt: chrono::DateTime<Tz>) -> JsonValue
where
    Tz::Offset: std::fmt::Display,
{
    json!({
        "range": {
            "@timestamp": {"gte": format_timestamp(dt)}
        }
    })
}

pub fn timestamp_lte_query<Tz: chrono::offset::TimeZone>(dt: chrono::DateTime<Tz>) -> JsonValue
where
    Tz::Offset: std::fmt::Display,
{
    json!({
        "range": {
            "@timestamp": {"lte": format_timestamp(dt)}
        }
    })
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

pub fn term_query(field: &str, value: &str) -> JsonValue {
    json!({"term": {field: value}})
}

pub fn exists_query(field: &str) -> JsonValue {
    json!({"exists": {"field": field}})
}

#[derive(Deserialize, Debug)]
pub struct ElasticResponse {
    pub hits: Option<serde_json::Value>,
    pub error: Option<ElasticResponseError>,
    pub status: Option<u64>,
    pub failures: Option<Vec<JsonValue>>,
    pub total: Option<u64>,
    pub updated: Option<u64>,

    #[serde(flatten)]
    pub other: std::collections::HashMap<String, JsonValue>,
}

#[derive(Deserialize, Debug)]
pub struct ElasticResponseError {
    pub reason: String,
    #[serde(flatten)]
    pub other: std::collections::HashMap<String, JsonValue>,
}
