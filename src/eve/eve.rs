// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::{self, DateTime};

pub trait Eve {
    fn datetime(&self) -> Option<DateTime>;
    fn has_tag(&self, tag: &str) -> bool;
}

impl Eve for serde_json::Value {
    fn datetime(&self) -> Option<DateTime> {
        if let serde_json::Value::String(timestamp) = &self["timestamp"] {
            if let Ok(dt) = datetime::parse(timestamp, None) {
                return Some(dt);
            }
        }
        None
    }

    fn has_tag(&self, tag: &str) -> bool {
        has_tag(self, tag)
    }
}

/// Ensure the event has a tags array.
pub(crate) fn ensure_has_tags(event: &mut serde_json::Value) {
    if event["tags"].as_array().is_none() {
        event["tags"] = serde_json::Value::Array(vec![]);
    }
}

/// Ensure the event has an evebox object.
pub(crate) fn ensure_has_evebox(event: &mut serde_json::Value) {
    if event["evebox"].as_object().is_none() {
        event["evebox"] = serde_json::Value::Object(serde_json::Map::new());
    }
}

pub(crate) fn has_tag(event: &serde_json::Value, tag: &str) -> bool {
    if let serde_json::Value::Array(tags) = &event["tags"] {
        for t in tags {
            if let serde_json::Value::String(t) = t {
                if t == tag {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn ensure_has_history(event: &mut serde_json::Value) {
    if let serde_json::Value::Null = &event["evebox"] {
        event["evebox"] = json!({});
    }
    if let serde_json::Value::Null = &event["evebox"]["history"] {
        event["evebox"]["history"] = json!([]);
    }
}
