// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! The idea here is an ingest "pipeline" for events. Its not really a
//! pipeline, but the idea is the same. Take in event event, and
//! return a modified, enriched, enhanced event.

use crate::sqlite::configdb::FilterEntry;

use std::collections::HashSet;

#[derive(Default, Debug)]
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
