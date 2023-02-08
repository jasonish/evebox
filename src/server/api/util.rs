// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use anyhow::Result;
use std::time::Duration;
use std::time::UNIX_EPOCH;

/// Parse a string representing a duration.
///
/// This is a wrapper around humantime with special handlers for "",
/// "all" and "*" which will return the duration since the unix epoch.
pub(crate) fn parse_duration(duration: &str) -> Result<Duration> {
    match duration {
        "" | "all" | "*" => Ok(UNIX_EPOCH.elapsed()?),
        _ => Ok(humantime::parse_duration(duration)?),
    }
}
