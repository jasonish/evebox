// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::{self, DateTime};

pub trait Eve {
    fn datetime(&self) -> Option<DateTime>;
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
}

pub(crate) fn add_evebox_metadata(event: &mut serde_json::Value, filename: Option<String>) {
    if let serde_json::Value::Null = event["evebox"] {
        event["evebox"] = serde_json::json!({});
    }
    if let serde_json::Value::Object(_) = &event["evebox"] {
        if let Some(filename) = filename {
            event["evebox"]["filename"] = filename.into();
        }
    }

    // Add a tags object.
    event["tags"] = serde_json::json!([]);
}

pub(crate) fn ensure_has_history(event: &mut serde_json::Value) {
    if let serde_json::Value::Null = &event["evebox"] {
        event["evebox"] = json!({});
    }
    if let serde_json::Value::Null = &event["evebox"]["history"] {
        event["evebox"]["history"] = json!([]);
    }
}
