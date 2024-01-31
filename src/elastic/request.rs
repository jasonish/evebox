// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use time::macros::format_description;

pub trait Request {
    fn push_filter(&mut self, filter: serde_json::Value);
    fn size(&mut self, size: u64);
    fn set_filters(&mut self, filters: Vec<serde_json::Value>);
}

impl Request for serde_json::Value {
    fn push_filter(&mut self, filter: serde_json::Value) {
        if let Some(filters) = self["query"]["bool"]["filter"].as_array_mut() {
            filters.push(filter);
        }
    }

    fn size(&mut self, size: u64) {
        self["size"] = size.into();
    }

    fn set_filters(&mut self, filters: Vec<serde_json::Value>) {
        self["query"]["bool"]["filter"] = filters.into();
    }
}

pub fn new_request() -> serde_json::Value {
    json!({
        "query": {
            "bool": {
                "filter": [],
            }
        }
    })
}

/// Format: "%Y-%m-%dT%H:%M:%S.%3fZ";
pub fn format_datetime(dt: &time::OffsetDateTime) -> String {
    let format =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
    dt.to_offset(time::UtcOffset::UTC).format(&format).unwrap()
}

pub fn term_filter(field: &str, value: &str) -> serde_json::Value {
    json!({"term": {field: value}})
}

pub fn exists_filter(field: &str) -> serde_json::Value {
    json!({"exists": {"field": field}})
}

pub fn range_lte_filter(field: &str, value: &str) -> serde_json::Value {
    json!({"range": {field: {"lte": value}}})
}

pub fn range_gte_filter(field: &str, value: &str) -> serde_json::Value {
    json!({"range": {field: {"gte": value}}})
}

pub fn timestamp_gte_filter(dt: &time::OffsetDateTime) -> serde_json::Value {
    range_gte_filter("@timestamp", &format_datetime(dt))
}

pub fn timestamp_lte_filter(dt: &time::OffsetDateTime) -> serde_json::Value {
    range_lte_filter("@timestamp", &format_datetime(dt))
}
