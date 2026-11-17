// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::sync::LazyLock;
use std::time::SystemTime;

use serde::{Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

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
    pub datetime: OffsetDateTime,
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
        serializer.serialize_str(&self.to_eve())
    }
}

// Preserve fractional seconds in groups of three digits, omitted for whole
// seconds. JSON uses Z for UTC; Display uses an explicit +00:00 offset.
fn format_rfc3339(datetime: OffsetDateTime, use_z: bool) -> String {
    let mut value = datetime
        .format(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second]"
        ))
        .unwrap();
    let nanos = datetime.nanosecond();
    if nanos != 0 {
        if nanos.is_multiple_of(1_000_000) {
            value.push_str(&format!(".{:03}", nanos / 1_000_000));
        } else if nanos.is_multiple_of(1_000) {
            value.push_str(&format!(".{:06}", nanos / 1_000));
        } else {
            value.push_str(&format!(".{nanos:09}"));
        }
    }
    if use_z && datetime.offset() == UtcOffset::UTC {
        value.push('Z');
    } else {
        value.push_str(
            &datetime
                .format(format_description!(
                    "[offset_hour sign:mandatory]:[offset_minute]"
                ))
                .unwrap(),
        );
    }
    value
}

pub(crate) fn serialize_rfc3339<S>(
    datetime: &OffsetDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format_rfc3339(*datetime, true))
}

pub(crate) fn serialize_optional_rfc3339<S>(
    datetime: &Option<OffsetDateTime>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match datetime {
        Some(datetime) => serializer.serialize_some(&format_rfc3339(*datetime, true)),
        None => serializer.serialize_none(),
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&format_rfc3339(self.datetime, false))
    }
}

impl std::ops::Sub<std::time::Duration> for DateTime {
    type Output = Self;

    fn sub(self, rhs: std::time::Duration) -> Self::Output {
        (self.datetime - rhs).into()
    }
}

impl std::cmp::PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.datetime.partial_cmp(&other.datetime)
    }
}

impl DateTime {
    pub(crate) fn now() -> Self {
        OffsetDateTime::now_utc().into()
    }

    pub(crate) fn from_seconds(seconds: i64) -> Self {
        OffsetDateTime::from_unix_timestamp(seconds).unwrap().into()
    }

    pub(crate) fn from_nanos(nanos: i64) -> Self {
        OffsetDateTime::from_unix_timestamp_nanos(nanos.into())
            .unwrap()
            .into()
    }

    pub(crate) fn to_rfc3339_utc(&self) -> String {
        self.datetime
            .to_offset(UtcOffset::UTC)
            .format(format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
            ))
            .unwrap()
    }

    pub(crate) fn to_eve(&self) -> String {
        self.datetime
            .format(format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6][offset_hour sign:mandatory][offset_minute]"))
            .unwrap()
    }

    /// RFC3339 in UTC, with millisecond precision, for Elasticsearch.
    pub(crate) fn to_elastic(&self) -> String {
        self.datetime
            .to_offset(UtcOffset::UTC)
            .format(format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
            ))
            .unwrap()
    }

    pub(crate) fn to_seconds(&self) -> i64 {
        self.datetime.unix_timestamp()
    }

    pub(crate) fn to_nanos(&self) -> i64 {
        self.datetime.unix_timestamp_nanos().try_into().unwrap()
    }

    pub(crate) fn micros_part(&self) -> i64 {
        self.datetime.microsecond().into()
    }

    pub(crate) fn yyyymmdd(&self, sep: &str) -> String {
        format!(
            "{:04}{sep}{:02}{sep}{:02}",
            self.datetime.year(),
            self.datetime.month() as u8,
            self.datetime.day()
        )
    }

    pub(crate) fn to_systemtime(&self) -> SystemTime {
        self.datetime.into()
    }

    pub(crate) fn sub(&self, rhs: time::Duration) -> Self {
        (self.datetime - rhs).into()
    }

    pub(crate) fn add(&self, rhs: time::Duration) -> Self {
        (self.datetime + rhs).into()
    }

    pub(crate) fn checked_sub_std(&self, rhs: std::time::Duration) -> Result<Self, ParseError> {
        let duration = time::Duration::try_from(rhs)
            .map_err(|_| ParseError("duration out of range".into()))?;
        self.datetime
            .checked_sub(duration)
            .map(Into::into)
            .ok_or_else(|| ParseError("timestamp out of range".into()))
    }
}

impl From<OffsetDateTime> for DateTime {
    fn from(datetime: OffsetDateTime) -> Self {
        DateTime { datetime }
    }
}

pub(crate) fn parse(input: &str, tz_offset: Option<&str>) -> Result<DateTime, ParseError> {
    if let Ok(ts) = OffsetDateTime::parse(input, &Rfc3339) {
        return Ok(ts.into());
    }

    // Suricata's EVE format uses compact offsets. Keep this common ingestion
    // path out of the partial-date normalization below.
    if let Ok(ts) = OffsetDateTime::parse(
        input,
        format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond][offset_hour sign:mandatory][offset_minute]"
        ),
    ) {
        return Ok(ts.into());
    }

    // The query language also accepts partial and compact dates.
    static PARTIAL: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
        r"^(\d{4})-?(\d{2})?-?(\d{2})?T?(\d{2})?:?(\d{2})?:?(\d{2})?(?:\.(\d+))?([+\-]\d{2}:?\d{2}|Z)?$",
    ).unwrap()
    });
    if let Some(c) = PARTIAL.captures(input) {
        let year = c.get(1).map_or("", |m| m.as_str());
        let month = c.get(2).map_or("01", |m| m.as_str());
        let day = c.get(3).map_or("01", |m| m.as_str());
        let hour = c.get(4).map_or("00", |m| m.as_str());
        let minute = c.get(5).map_or("00", |m| m.as_str());
        let second = c.get(6).map_or("00", |m| m.as_str());
        let subs = c.get(7).map_or("0", |m| m.as_str());
        let offset = c.get(8).map_or(tz_offset.unwrap_or("Z"), |m| m.as_str());
        let offset = if offset.len() == 5
            && offset.is_ascii()
            && matches!(offset.as_bytes()[0], b'+' | b'-')
        {
            format!("{}:{}", &offset[..3], &offset[3..])
        } else {
            offset.to_string()
        };
        let fixed = format!("{year}-{month}-{day}T{hour}:{minute}:{second}.{subs}{offset}");
        if let Ok(ts) = OffsetDateTime::parse(&fixed, &Rfc3339) {
            return Ok(ts.into());
        }
    }
    Err(ParseError("invalid format".to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn timestamp_formats_and_precision() {
        for input in [
            "2024-05-17T15:34:08.828074-0600",
            "2024-05-17T15:34:08.828074-06:00",
        ] {
            let dt = parse(input, None).unwrap();
            assert_eq!(dt.to_eve(), "2024-05-17T15:34:08.828074-0600");
            assert_eq!(dt.to_elastic(), "2024-05-17T21:34:08.828Z");
            assert_eq!(dt.to_rfc3339_utc(), "2024-05-17T21:34:08.828074Z");
            assert_eq!(dt.to_string(), "2024-05-17T15:34:08.828074-06:00");
            assert_eq!(serde_json::to_value(&dt).unwrap(), dt.to_eve());
        }
        let dt = parse("2024-05-17T21:34:08.123456789Z", None).unwrap();
        assert_eq!(DateTime::from_nanos(dt.to_nanos()), dt);
        assert_eq!(dt.micros_part(), 123456);
        assert_eq!(dt.to_rfc3339_utc(), "2024-05-17T21:34:08.123456Z");
        assert_eq!(dt.yyyymmdd("."), "2024.05.17");
        assert_eq!(DateTime::from_nanos(-1).to_seconds(), -1);
        assert_eq!(DateTime::from_nanos(-1).micros_part(), 999999);
    }

    #[test]
    fn partial_and_compact_timestamps() {
        let ts0 = parse("2024-05-16T16:08:17.876423-0600", None).unwrap();
        let ts1 = parse("20240516T160817.876423-0600", None).unwrap();
        assert_eq!(ts0, ts1);
        for input in [
            "2024",
            "2024-05",
            "2024-05-16",
            "2024-05-16T16",
            "2024-05-16T16:08",
            "2024-05-16T16:08:17",
            "2024-05-16T16:08:17.876",
            "2024-05-16T16:08:17Z",
            "2024-05-16+0000",
        ] {
            assert!(parse(input, None).is_ok(), "{input}");
        }
        assert_eq!(
            parse("2024-05-16T16:08", Some("-0600")).unwrap(),
            parse("2024-05-16T22:08:00Z", None).unwrap()
        );
        assert_eq!(
            parse("2024-05-16T16:08", Some("-06:00")).unwrap(),
            parse("2024-05-16T22:08:00Z", None).unwrap()
        );
        for input in [
            "2024-13-01",
            "2024-02-30",
            "2024-05-16garbage",
            "",
            "not a timestamp",
        ] {
            assert!(parse(input, None).is_err(), "{input}");
        }
    }

    #[test]
    fn api_timestamp_representation() {
        #[derive(Serialize)]
        struct Row {
            #[serde(serialize_with = "serialize_rfc3339")]
            created_at: OffsetDateTime,
            #[serde(serialize_with = "serialize_optional_rfc3339")]
            last_seen: Option<OffsetDateTime>,
        }
        for (fraction, expected) in [
            ("", ""),
            (".1", ".100"),
            (".1234", ".123400"),
            (".1234567", ".123456700"),
        ] {
            let dt = parse(&format!("2024-01-01T00:00:00{fraction}Z"), None).unwrap();
            assert_eq!(
                dt.to_string(),
                format!("2024-01-01T00:00:00{expected}+00:00")
            );
            let expected = format!("2024-01-01T00:00:00{expected}Z");
            let row = Row {
                created_at: dt.datetime,
                last_seen: Some(dt.datetime),
            };
            assert_eq!(
                serde_json::to_value(&row).unwrap(),
                serde_json::json!({
                    "created_at": expected,
                    "last_seen": expected,
                })
            );
            let row = Row {
                last_seen: None,
                ..row
            };
            assert!(serde_json::to_value(&row).unwrap()["last_seen"].is_null());
        }
        let dt = parse("2024-01-01T00:00:00.123456-05:30", None).unwrap();
        let row = Row {
            created_at: dt.datetime,
            last_seen: Some(dt.datetime),
        };
        assert_eq!(
            serde_json::to_value(&row).unwrap(),
            serde_json::json!({
                "created_at": "2024-01-01T00:00:00.123456-05:30",
                "last_seen": "2024-01-01T00:00:00.123456-05:30",
            })
        );
    }

    #[tokio::test]
    async fn reads_existing_sqlite_timestamps() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        for (input, nanos) in [
            ("2024-01-01 00:00:00", 0),
            ("2024-01-01T00:00:00+00:00", 0),
            ("2024-01-01T01:00:00+01:00", 0),
            ("2024-01-01T00:00:00Z", 0),
            ("2024-01-01T00:00:00.123456789+00:00", 123_456_789),
            ("2024-01-01 00:00:00.123456", 123_456_000),
        ] {
            let dt: OffsetDateTime = sqlx::query_scalar("SELECT ?")
                .bind(input)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(dt.unix_timestamp(), 1_704_067_200, "{input}");
            assert_eq!(dt.nanosecond(), nanos, "{input}");
            let roundtrip: OffsetDateTime = sqlx::query_scalar("SELECT ?")
                .bind(dt)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(dt, roundtrip);
        }
    }

    #[test]
    fn oversized_duration_is_an_error() {
        assert!(
            DateTime::now()
                .checked_sub_std(std::time::Duration::MAX)
                .is_err()
        );
    }
}
