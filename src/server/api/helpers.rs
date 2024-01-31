// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::ApiError;
use crate::prelude::*;

use std::collections::HashMap;
use std::ops::Sub;
use std::str::FromStr;

pub fn mints_from_time_range(
    ts: Option<String>,
    now: Option<&time::OffsetDateTime>,
) -> Result<Option<time::OffsetDateTime>, ApiError> {
    if let Some(time_range) = &ts {
        let duration = humantime::Duration::from_str(time_range)
            .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
        let now = now.copied().unwrap_or_else(time::OffsetDateTime::now_utc);
        let mints = now.sub(*duration.as_ref());
        Ok(Some(mints))
    } else {
        Ok(None)
    }
}

pub fn log_unknown_parameters(handler: &str, map: &HashMap<String, String>) {
    for (key, val) in map {
        warn!("{}: unknown query string key/val: {}={}", handler, key, val);
    }
}
