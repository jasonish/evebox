// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::datetime::DateTime;

pub(crate) use client::Version;
pub(crate) use client::{Client, ClientBuilder};
pub(crate) use eventrepo::ElasticEventRepo;
pub(crate) use importer::ElasticEventSink;

pub(crate) mod autoarchive;
pub(crate) mod client;
pub(crate) mod eventrepo;
pub(crate) mod importer;
pub(crate) mod request;
pub(crate) mod retention;
pub(crate) mod util;

pub(crate) const TAG_ESCALATED: &str = "evebox.escalated";
pub(crate) const TAG_ARCHIVED: &str = "evebox.archived";
pub(crate) const TAG_AUTO_ARCHIVED: &str = "evebox.auto-archived";

pub(crate) const TAGS_ESCALATED: [&str; 1] = [TAG_ESCALATED];
pub(crate) const TAGS_ARCHIVED: [&str; 1] = [TAG_ARCHIVED];
pub(crate) const TAGS_AUTO_ARCHIVED: [&str; 2] = [TAG_ARCHIVED, TAG_AUTO_ARCHIVED];

pub(crate) enum HistoryType {
    Archived,
    AutoArchived,
    Escalated,
    Deescalated,
    Comment,
}

impl std::fmt::Display for HistoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HistoryType::Archived => write!(f, "archived"),
            HistoryType::AutoArchived => write!(f, "auto-archived"),
            HistoryType::Escalated => write!(f, "escalated"),
            HistoryType::Deescalated => write!(f, "de-escalated"),
            HistoryType::Comment => write!(f, "comment"),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub(crate) struct AlertQueryOptions {
    pub timestamp_gte: Option<DateTime>,
    pub query_string: Option<String>,
    pub tags: Vec<String>,
    pub sensor: Option<String>,
    pub timeout: Option<u64>,
}

#[derive(Serialize, Debug)]
pub(crate) struct HistoryEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub timestamp: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl HistoryEntry {
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

pub(crate) struct HistoryEntryBuilder {
    timestamp: DateTime,
    action: String,
    username: Option<String>,
    comment: Option<String>,
}

impl HistoryEntryBuilder {
    fn new(action: HistoryType) -> Self {
        Self {
            action: action.to_string(),
            timestamp: DateTime::now(),
            username: None,
            comment: None,
        }
    }

    pub fn new_archived() -> Self {
        Self::new(HistoryType::Archived)
    }

    pub fn new_auto_archived() -> Self {
        Self::new(HistoryType::AutoArchived)
    }

    pub fn new_escalate() -> Self {
        Self::new(HistoryType::Escalated)
    }

    pub fn new_deescalate() -> Self {
        Self::new(HistoryType::Deescalated)
    }

    pub fn new_comment() -> Self {
        Self::new(HistoryType::Comment)
    }

    pub fn username(mut self, username: Option<impl Into<String>>) -> Self {
        self.username = username.map(|u| u.into());
        self
    }

    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    pub fn build(self) -> HistoryEntry {
        HistoryEntry {
            username: self.username,
            timestamp: self.timestamp.to_rfc3339_utc(),
            action: self.action,
            comment: self.comment,
        }
    }
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
        .replace('>', "\\>");

    json!({
        "query_string": {
            "default_operator": "AND",
            "query": escaped,
            "lenient": true,
        }
    })
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ElasticResponse {
    pub hits: Option<serde_json::Value>,
    pub error: Option<ElasticResponseError>,
    pub updated: Option<u64>,
    pub aggregations: Option<serde_json::Value>,

    #[allow(dead_code)]
    #[serde(default)]
    pub took: u64,

    #[allow(dead_code)]
    #[serde(default)]
    pub timed_out: bool,

    #[allow(dead_code)]
    #[serde(flatten)]
    pub other: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ElasticResponseError {
    pub root_cause: Vec<RootCause>,
}

impl ElasticResponseError {
    pub(crate) fn first_reason(&self) -> String {
        self.root_cause
            .first()
            .map(|rc| rc.reason.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RootCause {
    #[serde(rename = "type")]
    pub cause_type: String,
    pub reason: String,
    pub header: serde_json::Value,
}
