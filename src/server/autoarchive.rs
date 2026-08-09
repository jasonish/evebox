// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! The idea here is an ingest "pipeline" for events. Its not really a
//! pipeline, but the idea is the same. Take in event event, and
//! return a modified, enriched, enhanced event.

use crate::sqlite::configdb::{EventFilter, FilterOperator};

use std::collections::HashSet;

#[derive(Default, Debug)]
pub(crate) struct AutoArchive {
    filters: HashSet<String>,
}

impl AutoArchive {
    pub(crate) fn add(&mut self, filter: &EventFilter) {
        if let Some(key) = Self::key(filter) {
            self.filters.insert(key);
        }
    }

    pub(crate) fn is_match(&self, event: &serde_json::Value) -> bool {
        self.filters.contains(&self.key4(event))
            || self.filters.contains(&self.key3(event))
            || self.filters.contains(&self.key1(event))
            || self.filters.contains(&self.sensor_sid_key(event))
    }

    pub(crate) fn key(filter: &EventFilter) -> Option<String> {
        let mut sensor = None;
        let mut src_ip = None;
        let mut dest_ip = None;
        let mut signature_id = None;

        for condition in &filter.conditions {
            if condition.op != FilterOperator::Eq {
                return None;
            }
            match condition.field.as_str() {
                "host" if sensor.is_none() => sensor = Some(condition.value.as_str()?),
                "src_ip" if src_ip.is_none() => src_ip = Some(condition.value.as_str()?),
                "dest_ip" if dest_ip.is_none() => dest_ip = Some(condition.value.as_str()?),
                "alert.signature_id" if signature_id.is_none() => {
                    signature_id = Some(condition.value.as_i64()?)
                }
                _ => return None,
            }
        }

        Some(format!(
            "{},{},{},{}",
            sensor.unwrap_or("*"),
            src_ip.unwrap_or("*"),
            dest_ip.unwrap_or("*"),
            signature_id?
        ))
    }

    pub(crate) fn has_key(&self, key: &str) -> bool {
        self.filters.contains(key)
    }

    pub(crate) fn remove(&mut self, filter: &EventFilter) {
        if let Some(key) = Self::key(filter) {
            self.filters.remove(&key);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::configdb::{FilterAction, FilterCondition, FilterEntry, FilterOperator};

    #[test]
    fn existing_filter_shapes_still_match() {
        let event = json!({
            "host": "fw-east",
            "src_ip": "10.1.1.1",
            "dest_ip": "10.2.2.2",
            "alert": {"signature_id": 42},
        });
        for (sensor, src_ip, dest_ip) in [
            (None, None, None),
            (None, Some("10.1.1.1"), Some("10.2.2.2")),
            (Some("fw-east"), None, None),
            (Some("fw-east"), Some("10.1.1.1"), Some("10.2.2.2")),
        ] {
            let entry = FilterEntry {
                sensor: sensor.map(str::to_string),
                src_ip: src_ip.map(str::to_string),
                dest_ip: dest_ip.map(str::to_string),
                signature_id: 42,
                comment: None,
            };
            let mut auto_archive = AutoArchive::default();
            auto_archive.add(&EventFilter::from(&entry));
            assert!(auto_archive.is_match(&event));
        }
    }

    #[test]
    fn future_filter_shapes_are_not_enabled() {
        let filter = EventFilter {
            action: FilterAction::Archive,
            conditions: vec![FilterCondition {
                field: "src_ip".to_string(),
                op: FilterOperator::Eq,
                value: "10.1.1.1".into(),
            }],
        };
        assert!(AutoArchive::key(&filter).is_none());

        let filter = EventFilter {
            action: FilterAction::Archive,
            conditions: vec![
                FilterCondition {
                    field: "alert.signature_id".to_string(),
                    op: FilterOperator::Eq,
                    value: 42.into(),
                },
                FilterCondition {
                    field: "dns.queries.rrname".to_string(),
                    op: FilterOperator::Eq,
                    value: "example.com".into(),
                },
            ],
        };
        assert!(AutoArchive::key(&filter).is_none());
    }
}
