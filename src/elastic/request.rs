// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub trait Request {
    fn push_filter(&mut self, filter: serde_json::Value);
    fn size(&mut self, size: u64);
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

pub fn range_lt_filter(field: &str, value: &str) -> serde_json::Value {
    json!({"range": {field: {"lt": value}}})
}

pub fn range_gt_filter(field: &str, value: &str) -> serde_json::Value {
    json!({"range": {field: {"gt": value}}})
}

pub fn timestamp_gte_filter(dt: &crate::datetime::DateTime) -> serde_json::Value {
    range_gte_filter("@timestamp", &dt.to_elastic())
}

pub fn timestamp_lte_filter(dt: &crate::datetime::DateTime) -> serde_json::Value {
    range_lte_filter("@timestamp", &dt.to_elastic())
}

pub fn timestamp_gt_filter(dt: &crate::datetime::DateTime) -> serde_json::Value {
    range_gt_filter("@timestamp", &dt.to_elastic())
}

pub fn timestamp_lt_filter(dt: &crate::datetime::DateTime) -> serde_json::Value {
    range_lt_filter("@timestamp", &dt.to_elastic())
}
