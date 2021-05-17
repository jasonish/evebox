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

use super::ApiError;
use crate::prelude::*;

use std::collections::HashMap;
use std::str::FromStr;

type DateTime = chrono::DateTime<chrono::Utc>;

pub fn mints_from_time_range(
    ts: Option<String>,
    now: Option<&DateTime>,
) -> Result<Option<DateTime>, ApiError> {
    if let Some(time_range) = &ts {
        let duration = humantime::Duration::from_str(time_range)
            .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
        let duration = chrono::Duration::from_std(*duration.as_ref())
            .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
        let mints = now
            .unwrap_or(&chrono::Utc::now())
            .checked_sub_signed(duration)
            .ok_or_else(|| ApiError::TimeRangeParseError(time_range.to_string()))?;
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
