// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

use serde::Deserialize;
use std::ops::Sub;
use std::str::FromStr;

use crate::server::api::ApiError;

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
    pub tz_offset: Option<String>,

    #[serde(flatten)]
    pub other: std::collections::HashMap<String, String>,
}

impl GenericQuery {
    pub fn mints_from_time_range(
        &self,
        now: &time::OffsetDateTime,
    ) -> Result<Option<time::OffsetDateTime>, ApiError> {
        if let Some(time_range) = &self.time_range {
            if time_range == "0s" {
                return Ok(None);
            }
            let duration = humantime::Duration::from_str(time_range)
                .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
            let mints = now.sub(*duration.as_ref());
            Ok(Some(mints))
        } else {
            Ok(None)
        }
    }
}
