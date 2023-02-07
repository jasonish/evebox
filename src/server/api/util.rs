// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

pub(crate) fn parse_duration(
    duration: &str,
) -> Result<std::time::Duration, humantime::DurationError> {
    humantime::parse_duration(duration)
}
