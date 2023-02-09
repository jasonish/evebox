// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::{escaped, tag, take_while1};
use nom::character::complete::{multispace0, one_of};
use nom::sequence::{delimited, preceded};
use nom::IResult;

#[derive(Debug, PartialEq, Eq)]
pub enum Element {
    /// Bare string.
    String(String),
    /// A key value pair, (eg: alert.signature_id:222222)
    KeyVal(String, String),
    /// A timestamp specified with @before.
    BeforeTimestamp(time::OffsetDateTime),
    /// A timestamp specified with @after.
    AfterTimestamp(time::OffsetDateTime),
    /// IP address specified with @ip which is used to match on the
    /// source or destination IP address.
    Ip(String),
}

pub fn parse(qs: &str, _tz_offset: Option<&str>) -> Result<Vec<Element>> {
    let mut elements = vec![];
    let (_, tokens) = tokenize(qs).map_err(|err| {
        anyhow!(
            "Failed to tokenize query string: error={}, input={}",
            err,
            qs
        )
    })?;

    // Now post-process the simple elements into higher level elements
    // like @ip, @before, etc.
    for token in tokens {
        match token {
            Element::String(_) => {
                elements.push(token);
            }
            Element::KeyVal(ref key, ref val) => match key.as_ref() {
                "@ip" => elements.push(Element::Ip(val.to_string())),
                _ => elements.push(token),
            },
            _ => bail!("Unexpected element in tokenized query string: {:?}", token),
        }
    }
    Ok(elements)
}

/// First pass of parsing, this tokenizes the query string into the
/// basic elements of plain strings, or key value pairs.
pub fn tokenize(mut input: &str) -> IResult<&str, Vec<Element>> {
    let mut tokens = vec![];
    loop {
        if input.is_empty() {
            break;
        }
        let (next, _) = multispace0(input)?;
        let (next, token) = alt((parse_quoted, parse_string))(next)?;
        if next.starts_with(':') {
            let (next, value) = preceded(tag(":"), alt((parse_quoted, parse_string)))(next)?;
            tokens.push(Element::KeyVal(token.to_string(), value.to_string()));
            input = next;
        } else {
            tokens.push(Element::String(token.to_string()));
            input = next;
        }
    }
    Ok((input, tokens))
}

/// Parse a quote string.
///
/// Parsed and consumes up to the first non-escaped quote. Leading and terminating quotes are
/// discarded.
fn parse_quoted(input: &str) -> IResult<&str, &str> {
    let parse_escaped = escaped(
        take_while1(|c| c != '\\' && c != '"'),
        '\\',
        one_of("\"\\"), // Note, this will not detect invalid unicode escapes.
    );
    let mut parse_quoted = delimited(tag("\""), parse_escaped, tag("\""));
    parse_quoted(input)
}

fn parse_string(input: &str) -> IResult<&str, &str> {
    let mut parse_escaped = escaped(
        take_while1(|c| c != '\\' && c != ':' && c != ' '),
        '\\',
        one_of(":\\"), // Note, this will not detect invalid unicode escapes.
    );
    parse_escaped(input)
}

pub fn parse_timestamp(timestamp: &str, offset: Option<&str>) -> Result<time::OffsetDateTime> {
    let timestamp = timestamp_preprocess(timestamp, offset)?;
    Ok(crate::eve::parse_eve_timestamp(&timestamp)?)
}

/// Preprocesses a user supplied timestamp into something that can be parsed by the stricter
/// EVE timestamp format parser.
///
/// Requires at last a year.
fn timestamp_preprocess(s: &str, offset: Option<&str>) -> Result<String> {
    let default_offset = offset.unwrap_or("+0000");
    let re = regex::Regex::new(
        r"^(\d{4})-?(\d{2})?-?(\d{2})?[T ]?(\d{2})?:?(\d{2})?:?(\d{2})?(\.(\d+))?(([+\-]\d{4})|Z)?$",
    )
    .unwrap();
    if let Some(c) = re.captures(s) {
        let year = c.get(1).map_or("", |m| m.as_str());
        let month = c.get(2).map_or("01", |m| m.as_str());
        let day = c.get(3).map_or("01", |m| m.as_str());
        let hour = c.get(4).map_or("00", |m| m.as_str());
        let minute = c.get(5).map_or("00", |m| m.as_str());
        let second = c.get(6).map_or("00", |m| m.as_str());
        let subs = c.get(8).map_or("0", |m| m.as_str());
        let offset = c.get(9).map_or(default_offset, |m| m.as_str());
        Ok(format!(
            "{}-{}-{}T{}:{}:{}.{}{}",
            year,
            month,
            day,
            hour,
            minute,
            second,
            subs,
            if offset == "Z" { "+0000" } else { offset },
        ))
    } else {
        anyhow::bail!("Failed to create timestamp from {}", s);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_tokenize() {
        let (_rem, parsed) = tokenize(r#""ET DROP""#).unwrap();
        assert_eq!(parsed, vec![Element::String("ET DROP".to_string())]);

        let (_rem, parsed) = tokenize(r#"flow_id:1"#).unwrap();
        assert_eq!(
            parsed,
            vec![Element::KeyVal("flow_id".to_string(), "1".to_string())]
        );

        let (_rem, parsed) = tokenize(r#"alert.signature:"ET DROP Spamhaus""#).unwrap();
        assert_eq!(
            parsed,
            vec![Element::KeyVal(
                "alert.signature".to_string(),
                "ET DROP Spamhaus".to_string()
            )]
        );

        let (_rem, parsed) = tokenize(r#"flow:1 alert.signature:"ET DROP Spamhaus""#).unwrap();
        assert_eq!(
            parsed,
            vec![
                Element::KeyVal("flow".to_string(), "1".to_string()),
                Element::KeyVal(
                    "alert.signature".to_string(),
                    "ET DROP Spamhaus".to_string()
                )
            ]
        );

        let (_rem, parsed) =
            tokenize(r#"flow:1 alert.signature:"ET DROP Spamhaus" "SOME Quoted String""#).unwrap();
        assert_eq!(
            parsed,
            vec![
                Element::KeyVal("flow".to_string(), "1".to_string()),
                Element::KeyVal(
                    "alert.signature".to_string(),
                    "ET DROP Spamhaus".to_string()
                ),
                Element::String("SOME Quoted String".to_string())
            ]
        );

        let (_rem, parsed) =
            tokenize(r#"flow:1 alert.signature:"ET DROP Spamhaus" bad'ly"formatted"#).unwrap();
        assert_eq!(
            parsed,
            vec![
                Element::KeyVal("flow".to_string(), "1".to_string()),
                Element::KeyVal(
                    "alert.signature".to_string(),
                    "ET DROP Spamhaus".to_string()
                ),
                Element::String(r#"bad'ly"formatted"#.to_string())
            ]
        );

        let (_rem, parsed) = tokenize(r#""no-end-quote"#).unwrap();
        assert_eq!(
            parsed,
            vec![Element::String(r#""no-end-quote"#.to_string())]
        );

        let (_, parsed) = tokenize("token asdf").unwrap();
        assert_eq!(
            &parsed,
            &[
                Element::String("token".to_string()),
                Element::String("asdf".to_string())
            ]
        );

        let (_, parsed) = tokenize("alert.signature:\"WPAD\" 10.16.1.1").unwrap();
        let first = &parsed[0];
        assert_eq!(
            first,
            &Element::KeyVal("alert.signature".to_string(), "WPAD".to_string())
        );
    }

    #[test]
    fn test_parse_string() {
        let (rem, parsed) = parse_string("alert.signature_id").unwrap();
        assert_eq!(rem, "");
        assert_eq!(parsed, "alert.signature_id");

        let (rem, parsed) = parse_string("alert.signature_id:").unwrap();
        assert_eq!(rem, ":");
        assert_eq!(parsed, "alert.signature_id");

        let (rem, parsed) = parse_string("alert\\:signature_id:").unwrap();
        assert_eq!(rem, ":");
        assert_eq!(parsed, "alert\\:signature_id");

        let (rem, parsed) = parse_string("one two three").unwrap();
        assert_eq!(parsed, "one");
        assert_eq!(rem, " two three");
    }

    #[test]
    fn test_parse_quoted_string() {
        let (rem, parsed) = parse_quoted(r#""Testing\" asdf""#).unwrap();
        assert_eq!(rem, "");
        assert_eq!(parsed, r#"Testing\" asdf"#);

        let (rem, parsed) = parse_quoted(r#""Testing" and some remainder"#).unwrap();
        assert_eq!(parsed, "Testing");
        assert_eq!(rem, " and some remainder");
    }

    #[test]
    fn test_timestamp_preprocess() {
        assert_eq!(
            timestamp_preprocess("2023", None).unwrap(),
            "2023-01-01T00:00:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01", None).unwrap(),
            "2023-01-01T00:00:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01", None).unwrap(),
            "2023-01-01T01:00:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02", None).unwrap(),
            "2023-01-01T01:02:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01 01:02", None).unwrap(),
            "2023-01-01T01:02:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03", None).unwrap(),
            "2023-01-01T01:02:03.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123", None).unwrap(),
            "2023-01-01T01:02:03.123+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123-0600", None).unwrap(),
            "2023-01-01T01:02:03.123-0600"
        );

        // Timezone offset without sub-seconds.
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03-0600", None).unwrap(),
            "2023-01-01T01:02:03.0-0600"
        );

        // '-' in the date and ':' in the time are optional.
        assert_eq!(
            timestamp_preprocess("20230101T010203.123-0600", None).unwrap(),
            "2023-01-01T01:02:03.123-0600"
        );

        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123", Some("+1200")).unwrap(),
            "2023-01-01T01:02:03.123+1200"
        );

        // 'Z' as timezone offset.
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123Z", None).unwrap(),
            "2023-01-01T01:02:03.123+0000"
        );
    }
}
