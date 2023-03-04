// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

pub use super::EveReader;
use time::macros::format_description;

pub trait Eve {
    fn timestamp(&self) -> Option<time::OffsetDateTime>;
    fn add_tag(&mut self, tag: &str);
}

impl Eve for serde_json::Value {
    fn timestamp(&self) -> Option<time::OffsetDateTime> {
        if let serde_json::Value::String(ts) = &self["timestamp"] {
            if let Ok(dt) = parse_eve_timestamp(ts) {
                return Some(dt);
            }
        }
        None
    }

    fn add_tag(&mut self, tag: &str) {
        if let serde_json::Value::Null = self["tags"] {
            self["tags"] = serde_json::Value::Array(vec![]);
        }
        if let serde_json::Value::Array(ref mut tags) = &mut self["tags"] {
            tags.push(tag.into());
        }
    }
}

pub fn add_evebox_metadata(event: &mut serde_json::Value, filename: Option<String>) {
    if let serde_json::Value::Null = event["evebox"] {
        event["evebox"] = serde_json::json!({});
    }
    if let serde_json::Value::Object(_) = &event["evebox"] {
        if let Some(filename) = filename {
            event["evebox"]["filename"] = filename.into();
        }
    }

    // Add a tags object.
    event["tags"] = serde_json::json!([]);
}

/// Parser for Eve timestamps.
///
/// Example formats handled:
/// 2016-09-17T17:19:39.787733+0000
/// 2016-09-17T17:19:39.787733-0000
/// 2020-04-06T10:48:55.011800-0600
///
/// But also handle the format typically used in Elasticsearch as well.
/// 2020-04-06T10:48:55.011Z
pub fn parse_eve_timestamp(s: &str) -> Result<time::OffsetDateTime, time::error::Parse> {
    let format = format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond][offset_hour][offset_minute]"
    );
    let s = s.replace('Z', "-0000");
    let parsed = time::OffsetDateTime::parse(&s, &format)?;
    Ok(parsed)
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_eve_timestamp() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts = "2016-09-17T17:19:39.787733+0000";
        let _dt = super::parse_eve_timestamp(ts)?;
        let _st: std::time::SystemTime = _dt.into();

        let ts = "2016-09-17T17:19:39.787733-0000";
        let _dt = super::parse_eve_timestamp(ts)?;

        let ts = "2020-04-06T10:48:55.011800-0600";
        let _dt = super::parse_eve_timestamp(ts)?;

        let ts = "2020-04-06T10:48:55.011800+0600";
        let _dt = super::parse_eve_timestamp(ts)?;

        let ts: &str = "2020-04-06T10:48:55.011800Z";
        let dt = super::parse_eve_timestamp(ts);
        assert!(dt.is_ok());

        let ts: &str = "2020-04-06T10:48:55.011Z";
        let dt = super::parse_eve_timestamp(ts);
        assert!(dt.is_ok());

        Ok(())
    }

    #[test]
    fn test_timestamps() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts = "2020-04-06T10:48:55.011800-0600";
        let dt = crate::eve::parse_eve_timestamp(ts)?;
        let formatted = crate::sqlite::format_sqlite_timestamp(&dt);
        assert_eq!(formatted, "2020-04-06T16:48:55.011800+0000");

        Ok(())
    }

    #[test]
    fn test_from_nanos() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts = "2020-04-06T10:48:55.011800-0600";
        let dt = crate::eve::parse_eve_timestamp(ts)?;
        let nanos = dt.unix_timestamp_nanos();
        assert_eq!(nanos, 1586191735011800000);

        // Now convert nanos back to a datetime.
        let dt = time::OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap();
        let formatted = crate::sqlite::format_sqlite_timestamp(&dt);
        assert_eq!(formatted, "2020-04-06T16:48:55.011800+0000");

        Ok(())
    }
}
