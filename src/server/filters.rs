// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use serde::Deserialize;

#[derive(Deserialize, Debug, Default)]
pub struct GenericQuery {
    pub tags: Option<String>,
    pub time_range: Option<String>,
    pub query_string: Option<String>,
    pub min_timestamp: Option<String>,
    pub max_timestamp: Option<String>,
    pub order: Option<String>,
    pub event_type: Option<String>,
    pub sort_by: Option<String>,
    pub size: Option<u64>,
    pub interval: Option<String>,
    pub address_filter: Option<String>,
    pub dns_type: Option<String>,
    pub agg: Option<String>,
    pub sensor_name: Option<String>,
    pub tz_offset: Option<String>,
}
