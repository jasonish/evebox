// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;

use super::util::parse_duration;
use crate::error::AppError;
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

    // Replaces min_timestamp and max_timestamp.
    pub from: Option<String>,
    pub to: Option<String>,

    pub timeout: Option<u64>,
}

impl TimeRange {
    pub fn parse_time_range(&self) -> Result<std::time::Duration, AppError> {
        parse_duration(&self.0).map_err(|_err| AppError::BadRequest("time_range".to_string()))
    }

    pub fn parse_time_range_as_min_timestamp(&self) -> Result<DateTime, AppError> {
        let range: std::time::Duration = self.parse_time_range()?;
        DateTime::now()
            .checked_sub_std(range)
            .map_err(|err| AppError::BadRequest(format!("time_range: {err}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_and_out_of_range_durations_are_bad_requests() {
        for input in ["2 hours", "-1s", "10000000d", "18446744073709551616s"] {
            assert!(
                matches!(
                    TimeRange(input.to_string()).parse_time_range_as_min_timestamp(),
                    Err(AppError::BadRequest(_))
                ),
                "{input}"
            );
        }
    }
}
