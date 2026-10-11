// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::datetime::DateTime;
use crate::prelude::*;

pub(crate) use client::Version;
pub(crate) use client::compatibility_warning;
pub(crate) use client::{Client, ClientBuilder};
pub(crate) use eventrepo::ElasticEventRepo;
pub(crate) use importer::ElasticEventSink;

pub(crate) mod autoarchive;
pub(crate) mod client;
pub(crate) mod eventrepo;
pub(crate) mod fieldcaps;
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

#[derive(Serialize, Debug)]
pub(crate) struct HistoryEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub timestamp: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
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
    cause: Option<String>,
    comment: Option<String>,
}

impl HistoryEntryBuilder {
    fn new(action: HistoryType) -> Self {
        Self {
            action: action.to_string(),
            timestamp: DateTime::now(),
            username: None,
            cause: None,
            comment: None,
        }
    }

    pub(crate) fn new_archived() -> Self {
        Self::new(HistoryType::Archived)
    }

    pub(crate) fn new_auto_archived(cause: impl Into<String>) -> Self {
        let mut builder = Self::new(HistoryType::AutoArchived);
        builder.cause = Some(cause.into());
        builder
    }

    pub(crate) fn new_escalate() -> Self {
        Self::new(HistoryType::Escalated)
    }

    pub(crate) fn new_deescalate() -> Self {
        Self::new(HistoryType::Deescalated)
    }

    pub(crate) fn new_comment() -> Self {
        Self::new(HistoryType::Comment)
    }

    pub(crate) fn username(mut self, username: Option<impl Into<String>>) -> Self {
        self.username = username.map(|u| u.into());
        self
    }

    pub(crate) fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    pub(crate) fn build(self) -> HistoryEntry {
        HistoryEntry {
            username: self.username,
            timestamp: self.timestamp.to_rfc3339_utc(),
            action: self.action,
            cause: self.cause,
            comment: self.comment,
        }
    }
}

pub(crate) fn query_string_query(query_string: &str) -> serde_json::Value {
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

/// Warn about shards that failed a search. Elasticsearch still returns
/// 200 with the results of the other shards, which can be none, so
/// without this a failing query just looks like it matched nothing.
pub(crate) fn warn_shard_failures(query: &str, shards: &serde_json::Value) {
    let Some(failure) = shards["failures"].as_array().and_then(|v| v.first()) else {
        return;
    };
    let reason = &failure["reason"];
    warn!(
        "Elasticsearch {query} query failed on {} of {} shards, first failure: index={}, {}: {}",
        shards["failed"],
        shards["total"],
        failure["index"].as_str().unwrap_or("?"),
        reason["type"].as_str().unwrap_or("?"),
        reason["reason"].as_str().unwrap_or("?"),
    );
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ElasticResponse {
    pub hits: Option<serde_json::Value>,
    #[serde(rename = "_shards", default)]
    pub shards: serde_json::Value,
    pub error: Option<ElasticResponseError>,
    pub updated: Option<u64>,
    pub aggregations: Option<serde_json::Value>,

    #[serde(default)]
    pub took: u64,

    #[serde(default)]
    pub timed_out: bool,
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
pub(crate) struct RootCause {
    pub reason: String,
}
