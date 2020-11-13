// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

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

pub fn term_filter(field: &str, value: &str) -> JsonValue {
    json!({"term": {field: value}})
}

pub fn exists_filter(field: &str) -> JsonValue {
    json!({"exists": {"field": field}})
}
