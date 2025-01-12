// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! The idea here is an ingest "pipeline" for events. Its not really a
//! pipeline, but the idea is the same. Take in event event, and
//! return a modified, enriched, enhanced event.

use crate::{prelude::*, sqlite::configdb::FilterEntry};

use std::collections::HashSet;

#[derive(Default, Clone)]
pub(crate) struct IngestPipeline {
    pub archive_filters: Arc<RwLock<AutoArchive>>,
}

impl IngestPipeline {
    pub fn new(archive_filters: Arc<RwLock<AutoArchive>>) -> Self {
        Self { archive_filters }
    }

    fn set_archive_tags(&self, event: &mut serde_json::Value) {
        let tags = &mut event["tags"]
            .as_array()
            .cloned()
            .unwrap_or_else(std::vec::Vec::new);
        tags.push("evebox.archived".into());
        tags.push("evebox.auto-archived".into());
        tags.push("evebox.auto-archived-by-server".into());
        event["tags"] = serde_json::Value::Array(tags.clone());
    }

    pub fn handle(&self, event: &mut serde_json::Value) {
        let filters = self.archive_filters.read().unwrap();
        if filters.is_match(event) {
            self.set_archive_tags(event);
        }
    }
}

#[derive(Default)]
pub(crate) struct AutoArchive {
    filters: HashSet<String>,
}

impl AutoArchive {
    pub fn add(&mut self, entry: &FilterEntry) {
        self.filters.insert(self.key(entry));
    }

    pub fn is_match(&self, event: &serde_json::Value) -> bool {
        self.filters.contains(&self.key4(event))
            || self.filters.contains(&self.key3(event))
            || self.filters.contains(&self.key1(event))
            || self.filters.contains(&self.sensor_sid_key(event))
    }

    fn key(&self, entry: &FilterEntry) -> String {
        format!(
            "{},{},{},{}",
            &entry.sensor.as_ref().map_or("*", |v| v),
            &entry.src_ip.as_ref().map_or("*", |v| v),
            &entry.dest_ip.as_ref().map_or("*", |v| v),
            entry.signature_id
        )
    }

    pub fn has_key(&self, key: &str) -> bool {
        self.filters.contains(key)
    }

    pub fn remove(&mut self, entry: &FilterEntry) {
        self.filters.remove(&self.key(entry));
    }

    // sensor, src_ip, dest_ip, signature_id
    fn key4(&self, event: &serde_json::Value) -> String {
        format!(
            "{},{},{},{}",
            event["host"].as_str().unwrap_or("*"),
            event["src_ip"].as_str().unwrap_or("*"),
            event["dest_ip"].as_str().unwrap_or("*"),
            event["alert"]["signature_id"].as_i64().unwrap_or(0)
        )
    }

    // src_ip, dest_ip, signature_id
    fn key3(&self, event: &serde_json::Value) -> String {
        format!(
            "*,{},{},{}",
            event["src_ip"].as_str().unwrap_or("*"),
            event["dest_ip"].as_str().unwrap_or("*"),
            event["alert"]["signature_id"].as_i64().unwrap_or(0)
        )
    }

    // signature_id
    fn key1(&self, event: &serde_json::Value) -> String {
        format!(
            "*,*,*,{}",
            event["alert"]["signature_id"].as_i64().unwrap_or(0)
        )
    }

    // sensor, signature_id
    fn sensor_sid_key(&self, event: &serde_json::Value) -> String {
        format!(
            "{},*,*,{}",
            event["host"].as_str().unwrap_or("*"),
            event["alert"]["signature_id"].as_i64().unwrap_or(0)
        )
    }
}
