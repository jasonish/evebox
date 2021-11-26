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
    pub fn from_str(input: &str) -> Result<GenericQuery, ApiError> {
        let mut query: GenericQuery =
            serde_urlencoded::from_str(input).map_err(|_| ApiError::QueryStringParseError)?;
        query.fixup();
        Ok(query)
    }

    pub fn from_string(input: String) -> Result<GenericQuery, ApiError> {
        let mut query: GenericQuery =
            serde_urlencoded::from_str(&input).map_err(|_| ApiError::QueryStringParseError)?;
        query.fixup();
        Ok(query)
    }

    fn fixup(&mut self) {
        if self.time_range.is_none() {
            self.time_range = self.other.get("timeRange").map(String::from);
            self.other.remove("timeRange");
        }
        if self.event_type.is_none() {
            self.event_type = self.other.get("eventType").map(String::from);
            self.other.remove("eventType");
        }
        if self.address_filter.is_none() {
            self.address_filter = self.other.get("addressFilter").map(String::from);
            self.other.remove("addressFilter");
        }
        if self.dns_type.is_none() {
            self.dns_type = self.other.get("dnsType").map(String::from);
            self.other.remove("dnsType");
        }
        if self.query_string.is_none() {
            self.query_string = self.other.get("queryString").map(String::from);
            self.other.remove("queryString");
        }
        if self.sensor_name.is_none() {
            self.sensor_name = self.other.get("sensorFilter").map(String::from);
            self.other.remove("sensorFilter");
        }
    }

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
