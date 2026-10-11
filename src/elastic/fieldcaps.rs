// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Field types of the event indices, from the field capabilities API.
//!
//! Dynamically mapped strings are indexed as analyzed `text` with an
//! exact `keyword` sub-field. Aggregations and exact-value filters have to
//! use the keyword sub-field, so the event repository looks fields up here
//! instead of relying on a fixed list.
//!
//! The table is refreshed periodically, and on demand when a request uses
//! a field it does not know, such as one from an event type that has just
//! started arriving.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::prelude::*;

use super::Client;

/// How often the table is refreshed in the background.
pub(crate) const REFRESH_INTERVAL: Duration = Duration::from_secs(300);

/// Minimum time between refreshes triggered by an unknown field, so
/// fields that do not exist anywhere don't trigger a refresh per request.
const MISS_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FieldKind {
    /// Analyzed text, with the name of its keyword sub-field if it has one.
    Text { keyword: Option<String> },
    /// Any other type (keyword, numbers, dates, IPs, ...), usable as is.
    Plain,
}

#[derive(Default)]
pub(crate) struct FieldCaps {
    fields: RwLock<Option<Arc<HashMap<String, FieldKind>>>>,
    /// Serializes refreshes and records when the last one was attempted.
    last_refresh: tokio::sync::Mutex<Option<Instant>>,
}

impl std::fmt::Debug for FieldCaps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.snapshot().map(|fields| fields.len());
        f.debug_struct("FieldCaps").field("fields", &count).finish()
    }
}

impl FieldCaps {
    fn snapshot(&self) -> Option<Arc<HashMap<String, FieldKind>>> {
        self.fields.read().unwrap().clone()
    }

    /// The kind of a field, or None if unknown or not loaded yet.
    pub(crate) fn get(&self, name: &str) -> Option<FieldKind> {
        self.snapshot()?.get(name).cloned()
    }

    /// The number of text fields with a keyword sub-field.
    pub(crate) fn keyword_count(&self) -> usize {
        self.snapshot()
            .map(|fields| {
                fields
                    .values()
                    .filter(|kind| matches!(kind, FieldKind::Text { keyword: Some(_) }))
                    .count()
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn set(&self, fields: HashMap<String, FieldKind>) {
        *self.fields.write().unwrap() = Some(Arc::new(fields));
    }

    /// Refresh the table now.
    pub(crate) async fn refresh(&self, client: &Client, index_pattern: &str) -> Result<()> {
        let mut last_refresh = self.last_refresh.lock().await;
        *last_refresh = Some(Instant::now());
        self.load(client, index_pattern).await
    }

    /// Refresh the table if it doesn't know some of the fields, unless a
    /// refresh was attempted recently. Errors are logged, as callers can
    /// carry on with the fallback field handling.
    pub(crate) async fn ensure(&self, client: &Client, index_pattern: &str, names: &[String]) {
        if !self.has_unknown(names) {
            return;
        }
        let mut last_refresh = self.last_refresh.lock().await;
        // Another request may have refreshed while this one waited.
        if !self.has_unknown(names) {
            return;
        }
        if let Some(last) = *last_refresh
            && last.elapsed() < MISS_REFRESH_INTERVAL
        {
            return;
        }
        *last_refresh = Some(Instant::now());
        debug!("Refreshing Elasticsearch field capabilities for fields {names:?}");
        if let Err(err) = self.load(client, index_pattern).await {
            debug!("Failed to refresh Elasticsearch field capabilities: {err:#}");
        }
    }

    fn has_unknown(&self, names: &[String]) -> bool {
        match self.snapshot() {
            Some(fields) => names.iter().any(|name| !fields.contains_key(name)),
            None => !names.is_empty(),
        }
    }

    async fn load(&self, client: &Client, index_pattern: &str) -> Result<()> {
        let path = format!(
            "{index_pattern}/_field_caps?fields=*&ignore_unavailable=true&allow_no_indices=true"
        );
        let response = client.get(&path)?.send().await?.error_for_status()?;
        let response: serde_json::Value = response.json().await?;
        let fields = parse(&response);
        debug!("Loaded {} Elasticsearch field capabilities", fields.len());
        *self.fields.write().unwrap() = Some(Arc::new(fields));
        Ok(())
    }
}

/// Build the field table from a `_field_caps` response.
pub(crate) fn parse(response: &serde_json::Value) -> HashMap<String, FieldKind> {
    let Some(fields) = response["fields"].as_object() else {
        return HashMap::new();
    };

    let types = |name: &str| -> HashSet<&str> {
        fields
            .get(name)
            .and_then(|types| types.as_object())
            .map(|types| {
                types
                    .keys()
                    .map(String::as_str)
                    .filter(|t| *t != "unmapped")
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut table = HashMap::new();
    for name in fields.keys() {
        let field_types = types(name);
        // Skip metadata fields, and objects which can't be queried directly.
        if name.starts_with('_')
            || field_types
                .iter()
                .all(|t| matches!(*t, "object" | "nested"))
        {
            continue;
        }
        let kind = if field_types.contains("text") {
            // With text in some indices and keyword in others, prefer the
            // keyword sub-field where it exists.
            let keyword = format!("{name}.keyword");
            let keyword = types(&keyword).contains("keyword").then_some(keyword);
            FieldKind::Text { keyword }
        } else {
            FieldKind::Plain
        };
        table.insert(name.clone(), kind);
    }
    table
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_field_caps() {
        let response = json!({
            "indices": ["logstash-2026.10.10", "logstash-2026.10.11"],
            "fields": {
                "_id": {"_id": {"type": "_id"}},
                "tls": {"object": {"type": "object"}},
                "tls.version": {"text": {"type": "text"}},
                "tls.version.keyword": {"keyword": {"type": "keyword"}},
                "http.url": {"text": {"type": "text"}},
                "flow.bytes_toserver": {"long": {"type": "long"}},
                "timestamp": {"date": {"type": "date"}},
                "src_ip": {"ip": {"type": "ip"}},
                "agent.name": {"keyword": {"type": "keyword"}},
                "mixed": {
                    "text": {"type": "text", "indices": ["logstash-2026.10.10"]},
                    "keyword": {"type": "keyword", "indices": ["logstash-2026.10.11"]},
                },
                "mixed.keyword": {
                    "keyword": {"type": "keyword", "indices": ["logstash-2026.10.10"]},
                    "unmapped": {"type": "unmapped", "indices": ["logstash-2026.10.11"]},
                },
            }
        });
        let table = parse(&response);

        assert_eq!(
            table["tls.version"],
            FieldKind::Text {
                keyword: Some("tls.version.keyword".to_string())
            }
        );
        assert_eq!(table["tls.version.keyword"], FieldKind::Plain);
        assert_eq!(table["http.url"], FieldKind::Text { keyword: None });
        assert_eq!(table["flow.bytes_toserver"], FieldKind::Plain);
        assert_eq!(table["timestamp"], FieldKind::Plain);
        assert_eq!(table["src_ip"], FieldKind::Plain);
        assert_eq!(table["agent.name"], FieldKind::Plain);
        assert_eq!(
            table["mixed"],
            FieldKind::Text {
                keyword: Some("mixed.keyword".to_string())
            }
        );
        assert!(!table.contains_key("_id"));
        assert!(!table.contains_key("tls"));
    }

    #[test]
    fn parse_unexpected_response() {
        assert!(parse(&json!({"error": "nope"})).is_empty());
    }
}
