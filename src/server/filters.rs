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

use serde::Deserialize;
use std::str::FromStr;

use crate::server::api::ApiError;

type DateTime = chrono::DateTime<chrono::Utc>;

#[derive(Deserialize, Debug, Default)]
pub struct GenericQuery {
    pub tags: Option<String>,
    pub time_range: Option<String>,
    pub query_string: Option<String>,
    pub min_ts: Option<String>,
    pub max_ts: Option<String>,
    pub order: Option<String>,
    pub event_type: Option<String>,
    pub sort_by: Option<String>,
    pub size: Option<u64>,
    pub interval: Option<String>,
    pub address_filter: Option<String>,
    pub dns_type: Option<String>,
    pub agg: Option<String>,
    pub sensor_name: Option<String>,

    #[serde(flatten)]
    pub other: std::collections::HashMap<String, String>,
}

impl GenericQuery {
    pub fn mints_from_time_range(&self, now: &DateTime) -> Result<Option<DateTime>, ApiError> {
        if let Some(time_range) = &self.time_range {
            if time_range == "0s" {
                return Ok(None);
            }
            let duration = humantime::Duration::from_str(time_range)
                .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
            let duration = chrono::Duration::from_std(*duration.as_ref())
                .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
            let mints = now
                .checked_sub_signed(duration)
                .ok_or_else(|| ApiError::TimeRangeParseError(time_range.to_string()))?;
            Ok(Some(mints))
        } else {
            Ok(None)
        }
    }
}
