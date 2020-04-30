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

use super::ApiError;
use crate::logger::log;
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
        log::warn!("{}: unknown query string key/val: {}={}", handler, key, val);
    }
}
