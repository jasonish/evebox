// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::{util::parse_duration, ApiError};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub(crate) struct TimeRange(String);

#[derive(Deserialize, Debug, Default)]
pub(crate) struct GenericQuery {
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
    pub tz_offset: Option<String>,
    pub sensor: Option<String>,
}

impl TimeRange {
    pub fn parse_time_range(&self) -> Result<std::time::Duration, ApiError> {
        parse_duration(&self.0).map_err(|_err| ApiError::bad_request("time_range"))
    }

    pub fn parse_time_range_as_min_timestamp(&self) -> Result<time::OffsetDateTime, ApiError> {
        let range = self.parse_time_range()?;
        Ok(time::OffsetDateTime::now_utc() - range)
    }
}
