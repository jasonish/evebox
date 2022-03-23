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

pub use super::EveReader;

pub type EveJson = serde_json::Value;

pub trait Eve {
    fn timestamp(&self) -> Option<chrono::DateTime<chrono::Utc>>;
    fn add_tag(&mut self, tag: &str);
}

impl Eve for EveJson {
    fn timestamp(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        if let EveJson::String(ts) = &self["timestamp"] {
            if let Ok(dt) = parse_eve_timestamp(ts) {
                return Some(dt);
            }
        }
        None
    }

    fn add_tag(&mut self, tag: &str) {
        if let EveJson::Null = self["tags"] {
            self["tags"] = EveJson::Array(vec![]);
        }
        if let EveJson::Array(ref mut tags) = &mut self["tags"] {
            tags.push(tag.into());
        }
    }
}

pub fn add_evebox_metadata(event: &mut EveJson, filename: Option<String>) {
    if let EveJson::Null = event["evebox"] {
        event["evebox"] = serde_json::json!({});
    }
    if let EveJson::Object(_) = &event["evebox"] {
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
pub fn parse_eve_timestamp(
    s: &str,
) -> Result<chrono::DateTime<chrono::Utc>, Box<dyn std::error::Error + Sync + Send>> {
    let s = s.replace('Z', "-0000");
    let dt = chrono::DateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.6f%z")?;
    Ok(dt.with_timezone(&chrono::Utc))
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_eve_timestamp() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts = "2016-09-17T17:19:39.787733+0000";
        let _dt = super::parse_eve_timestamp(ts)?;

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
}
