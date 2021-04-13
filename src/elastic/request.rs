// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::types::{DateTime, JsonValue};

const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S.%3fZ";

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

pub fn format_datetime(dt: DateTime) -> String {
    dt.format(TIME_FORMAT).to_string()
}

pub fn timestamp_gte_filter(dt: DateTime) -> JsonValue {
    json!({
        "range": {
            "@timestamp": {"gte": format_datetime(dt)}
        }
    })
}

pub fn timestamp_lte_filter(dt: DateTime) -> JsonValue {
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
