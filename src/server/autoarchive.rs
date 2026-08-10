// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! The idea here is an ingest "pipeline" for events. Its not really a
//! pipeline, but the idea is the same. Take in event event, and
//! return a modified, enriched, enhanced event.

use crate::sqlite::configdb::{EventFilter, FilterOperator};

#[derive(Default, Debug)]
pub(crate) struct AutoArchive {
    filters: Vec<EventFilter>,
}

impl AutoArchive {
    pub(crate) fn add(&mut self, filter: &EventFilter) {
        if !filter.conditions.is_empty() && !self.filters.contains(filter) {
            self.filters.push(filter.clone());
        }
    }

    pub(crate) fn is_match(&self, event: &serde_json::Value) -> bool {
        self.matching_filter(event).is_some()
    }

    pub(crate) fn matching_filter(&self, event: &serde_json::Value) -> Option<&EventFilter> {
        self.filters.iter().find(|filter| {
            filter
                .conditions
                .iter()
                .all(|condition| condition_matches(event, condition))
        })
    }

    pub(crate) fn contains(&self, filter: &EventFilter) -> bool {
        self.filters.contains(filter)
    }

    pub(crate) fn remove(&mut self, filter: &EventFilter) {
        self.filters.retain(|candidate| candidate != filter);
    }
}

fn condition_matches(
    event: &serde_json::Value,
    condition: &crate::sqlite::configdb::FilterCondition,
) -> bool {
    match condition.op {
        FilterOperator::Eq => value_matches(
            event,
            &condition.field.split('.').collect::<Vec<_>>(),
            &condition.value,
        ),
    }
}

fn value_matches(value: &serde_json::Value, path: &[&str], expected: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| value_matches(value, path, expected)),
        _ if path.is_empty() => value == expected,
        serde_json::Value::Object(values) => values
            .get(path[0])
            .is_some_and(|value| value_matches(value, &path[1..], expected)),
        _ => false,
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
    fn flexible_fields_and_arrays_match() {
        let src_ip_filter = EventFilter {
            action: FilterAction::Archive,
            conditions: vec![FilterCondition {
                field: "src_ip".to_string(),
                op: FilterOperator::Eq,
                value: "10.1.1.1".into(),
            }],
        };
        let mut auto_archive = AutoArchive::default();
        auto_archive.add(&src_ip_filter);
        assert!(auto_archive.is_match(&json!({"src_ip": "10.1.1.1"})));

        let dns_filter = EventFilter {
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
        let mut auto_archive = AutoArchive::default();
        auto_archive.add(&dns_filter);
        let event = json!({
            "alert": {"signature_id": 42},
            "dns": {
                "queries": [
                    {"rrname": "other.example"},
                    {"rrname": "example.com"},
                ],
            },
        });
        assert!(auto_archive.is_match(&event));

        let tls_filter = EventFilter {
            action: FilterAction::Archive,
            conditions: vec![FilterCondition {
                field: "tls.sni".to_string(),
                op: FilterOperator::Eq,
                value: "www.example.com".into(),
            }],
        };
        let mut auto_archive = AutoArchive::default();
        auto_archive.add(&tls_filter);
        assert!(auto_archive.is_match(&json!({
            "tls": {"sni": "www.example.com"},
        })));
        assert!(!auto_archive.is_match(&json!({
            "tls": {"sni": "other.example.com"},
        })));
    }
}
