// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::time::SystemTime;

use chrono::SecondsFormat;
use serde::{Serialize, Serializer};

pub(crate) type ChronoDateTime = chrono::DateTime<chrono::FixedOffset>;

#[derive(Debug)]
pub(crate) struct ParseError(String);

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DateTime {
    pub datetime: ChronoDateTime,
}

impl Default for DateTime {
    fn default() -> Self {
        DateTime::now()
    }
}

impl Serialize for DateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.to_eve();
        serializer.serialize_str(&s)
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.datetime.to_rfc3339())
    }
}

impl std::ops::Sub<std::time::Duration> for DateTime {
    type Output = Self;

    fn sub(self, rhs: std::time::Duration) -> Self::Output {
        let duration = chrono::Duration::from_std(rhs).unwrap();
        let new = self.datetime - duration;
        new.into()
    }
}

impl std::cmp::PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.datetime.partial_cmp(&other.datetime)
    }
}

impl DateTime {
    pub(crate) fn now() -> Self {
        DateTime {
            datetime: chrono::Utc::now().fixed_offset(),
        }
    }

    pub(crate) fn from_seconds(seconds: i64) -> Self {
        chrono::DateTime::from_timestamp(seconds, 0).unwrap().into()
    }

    pub(crate) fn from_nanos(nanos: i64) -> Self {
        chrono::DateTime::from_timestamp_nanos(nanos).into()
    }

    pub(crate) fn to_rfc3339_utc(&self) -> String {
        self.datetime
            .to_utc()
            .to_rfc3339_opts(SecondsFormat::Micros, true)
            .to_string()
    }

    pub(crate) fn to_eve(&self) -> String {
        self.datetime.format("%Y-%m-%dT%H:%M:%S.%6f%z").to_string()
    }

    /// Format to an Elasticsearch style format
    ///
    /// RFC3339 style, UTC, with Z of the timezone.
    pub(crate) fn to_elastic(&self) -> String {
        self.datetime
            .to_utc()
            .to_rfc3339_opts(SecondsFormat::Millis, true)
            .to_string()
    }

    /// Unix timestamp in seconds.
    pub(crate) fn to_seconds(&self) -> i64 {
        self.datetime.timestamp()
    }

    pub(crate) fn to_nanos(&self) -> i64 {
        // TODO, fix this. As we don't have any input with nano
        // precision, perhaps uses micros, multiply and return an
        // i128.
        self.datetime.timestamp_nanos_opt().unwrap()
    }

    /// Return the subsecond portion of the timestamp as micros.
    pub(crate) fn micros_part(&self) -> i64 {
        self.datetime.timestamp_subsec_micros() as i64
    }

    pub(crate) fn yyyymmdd(&self, sep: &str) -> String {
        self.datetime
            .format(&format!("%Y{sep}%m{sep}%d"))
            .to_string()
    }

    pub(crate) fn to_systemtime(&self) -> SystemTime {
        self.datetime.into()
    }

    pub(crate) fn sub(&self, rhs: chrono::Duration) -> Self {
        let new = self.datetime - rhs;
        new.into()
    }
}

impl From<chrono::DateTime<chrono::FixedOffset>> for DateTime {
    fn from(datetime: chrono::DateTime<chrono::FixedOffset>) -> Self {
        DateTime { datetime }
    }
}

impl From<chrono::DateTime<chrono::Utc>> for DateTime {
    fn from(datetime: chrono::DateTime<chrono::Utc>) -> Self {
        DateTime {
            datetime: datetime.fixed_offset(),
        }
    }
}

pub(crate) fn parse(input: &str, tz_offset: Option<&str>) -> Result<DateTime, ParseError> {
    // First attempt to parse it as is.
    if let Ok(ts) = input.parse::<chrono::DateTime<chrono::FixedOffset>>() {
        return Ok(ts.into());
    }

    let default_tz = tz_offset.unwrap_or("Z");

    // Now attempt to match it and fill in the missing bits. Requires at least a year.
    let re =
        r"^(\d{4})-?(\d{2})?-?(\d{2})?T?(\d{2})?:?(\d{2})?:?(\d{2})?(\.(\d+))?(([+\-]\d{4})|Z)?";
    let re = regex::Regex::new(re).unwrap();
    if let Some(c) = re.captures(input) {
        let year = c.get(1).map_or("", |m| m.as_str());
        let month = c.get(2).map_or("01", |m| m.as_str());
        let day = c.get(3).map_or("01", |m| m.as_str());
        let hour = c.get(4).map_or("00", |m| m.as_str());
        let minute = c.get(5).map_or("00", |m| m.as_str());
        let second = c.get(6).map_or("00", |m| m.as_str());
        let subs = c.get(8).map_or("0", |m| m.as_str());
        let offset = c.get(9).map_or(default_tz, |m| m.as_str());

        let fixed = format!("{year}-{month}-{day}T{hour}:{minute}:{second}.{subs}{offset}",);

        // Try again.
        if let Ok(ts) = fixed.parse::<chrono::DateTime<chrono::FixedOffset>>() {
            return Ok(ts.into());
        }
    }

    Err(ParseError("invalid format".to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    // Test some expectations of chrono.
    #[test]
    fn test_chrono() {
        let s = "2024-05-17T15:34:08.828074-0600";
        let dt = s.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-05-17T21:34:08.828074+00:00");

        let s = "2024-05-17T15:34:08.828074-06:00";
        let dt = s.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-05-17T21:34:08.828074+00:00");
        let dt: chrono::DateTime<chrono::FixedOffset> = dt.fixed_offset();
        assert_eq!(dt.to_rfc3339(), "2024-05-17T21:34:08.828074+00:00");

        let s = "2024-05-17T21:34:08.828074+00:00";
        let dt = s.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-05-17T21:34:08.828074+00:00");

        let s = "2024-05-17T21:34:08.828074-00:00";
        let dt = s.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-05-17T21:34:08.828074+00:00");

        let s = "2024-05-17T15:34:08.828074-06:00";
        let dt = s.parse::<chrono::DateTime<chrono::FixedOffset>>().unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-05-17T15:34:08.828074-06:00");
    }

    #[test]
    fn test_to_elastic() {
        let s = "2024-05-17T15:34:08.828074-06:00";
        let dt = s.parse::<chrono::DateTime<chrono::FixedOffset>>().unwrap();
        let dt = super::DateTime::from(dt);
        assert_eq!(dt.to_elastic(), "2024-05-17T21:34:08.828Z");
    }

    #[test]
    fn test_to_eve() {
        let s = "2024-05-17T15:34:08.828074-06:00";
        let dt = s.parse::<chrono::DateTime<chrono::FixedOffset>>().unwrap();
        let dt = super::DateTime::from(dt);
        assert_eq!(dt.to_eve(), "2024-05-17T15:34:08.828074-0600");
    }

    #[test]
    fn test_parse() {
        let ts0 = parse("2024-05-16T16:08:17.876423-0600", None).unwrap();
        let ts1 = parse("20240516T160817.876423-0600", None).unwrap();
        assert_eq!(ts0, ts1);

        let _ts = parse("2023-01-01T01:02:00.0+0000", None).unwrap();
        let _ts = parse("2024-05-16T16:08:17.876423+0600", None).unwrap();
        let _ts = parse("2024-05-16T16:08:17.876423Z", None).unwrap();
        let _ts = parse("2024", None).unwrap();
        let _ts = parse("2024-05", None).unwrap();
        let _ts = parse("2024-05-16", None).unwrap();
        let _ts = parse("2024-05-16T16", None).unwrap();
        let _ts = parse("2024-05-16T16:08", None).unwrap();
        let _ts = parse("2024-05-16T16:08:17", None).unwrap();
        let _ts = parse("2024-05-16T16:08:17.876", None).unwrap();
        let _ts = parse("2024-05-16T16:08:17Z", None).unwrap();
        let _ts = parse("2024-05-16+0000", None).unwrap();
    }

    #[test]
    fn test_sub() {
        let now = DateTime::now().datetime;
        let d = chrono::Duration::days(1);
        let _x = now - d;
    }
}
