// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

use crate::types::JsonValue;
use time::macros::format_description;

pub trait Request {
    fn push_filter(&mut self, filter: JsonValue);
    fn size(&mut self, size: u64);
    fn set_filters(&mut self, filters: Vec<JsonValue>);
}

impl Request for JsonValue {
    fn push_filter(&mut self, filter: JsonValue) {
        if let Some(filters) = self["query"]["bool"]["filter"].as_array_mut() {
            filters.push(filter);
        }
    }

    fn size(&mut self, size: u64) {
        self["size"] = size.into();
    }

    fn set_filters(&mut self, filters: Vec<JsonValue>) {
        self["query"]["bool"]["filter"] = filters.into();
    }
}

pub fn new_request() -> JsonValue {
    json!({
        "query": {
            "bool": {
                "filter": [],
            }
        }
    })
}

/// Format: "%Y-%m-%dT%H:%M:%S.%3fZ";
pub fn format_datetime(dt: time::OffsetDateTime) -> String {
    let format =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
    dt.to_offset(time::UtcOffset::UTC).format(&format).unwrap()
}

pub fn timestamp_gte_filter(dt: time::OffsetDateTime) -> JsonValue {
    json!({
        "range": {
            "@timestamp": {"gte": format_datetime(dt)}
        }
    })
}

pub fn timestamp_lte_filter(dt: time::OffsetDateTime) -> JsonValue {
    json!({
        "range": {
            "@timestamp": {"lte": format_datetime(dt)}
        }
    })
}

pub fn term_filter(field: &str, value: &str) -> JsonValue {
    json!({"term": {field: value}})
}

pub fn exists_filter(field: &str) -> JsonValue {
    json!({"exists": {"field": field}})
}

pub fn range_lte_filter(field: &str, value: &str) -> serde_json::Value {
    json!({"range": {field: {"lte": value}}})
}

pub fn range_gte_filter(field: &str, value: &str) -> serde_json::Value {
    json!({"range": {field: {"gte": value}}})
}
