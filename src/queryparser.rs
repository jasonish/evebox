// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use chrono::{DateTime, Utc};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_till},
    character::complete::multispace0,
    combinator::opt,
    IResult,
};

#[derive(Debug, Clone)]
pub struct QueryStringParseError(String);

impl std::error::Error for QueryStringParseError {}

impl std::fmt::Display for QueryStringParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "query string parse error: {}", self.0)
    }
}

impl From<nom::Err<nom::error::Error<&str>>> for QueryStringParseError {
    fn from(value: nom::Err<nom::error::Error<&str>>) -> Self {
        Self(format!("{:?}", value))
    }
}

impl From<String> for QueryStringParseError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub(crate) struct QueryParser {
    pub elements: Vec<QueryElement>,
}

impl QueryParser {
    pub fn new(elements: Vec<QueryElement>) -> Self {
        Self { elements }
    }

    /// Return the first QueryValue::From.
    pub fn first_from(&self) -> Option<&DateTime<Utc>> {
        for element in &self.elements {
            if let QueryValue::From(ts) = &element.value {
                return Some(ts);
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QueryValue {
    String(String),
    KeyValue(String, String),
    From(DateTime<Utc>),
    To(DateTime<Utc>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryElement {
    pub negated: bool,
    pub value: QueryValue,
}

/// Parse an EveBox query string into elements. A default timezone
/// offset is used as time specifiers are converted to time objects.
pub(crate) fn parse(
    input: &str,
    tz_offset: Option<&str>,
) -> Result<Vec<QueryElement>, QueryStringParseError> {
    let mut elements = vec![];
    let mut ptr = input;
    let mut token;
    let mut negated = false;

    while !ptr.is_empty() {
        (ptr, token) = parse_token(ptr)?;
        if token == "-" || token == "!" {
            negated = true;
        } else if ptr.starts_with(':') {
            let key = token.to_string();
            (ptr, token) = parse_value(&ptr[1..])?;

            match key.as_ref() {
                "@from" => {
                    let ts = parse_timestamp(&token, tz_offset)
                        .ok_or(format!("invalid time format: {}", &token))?;
                    elements.push(QueryElement {
                        negated: false,
                        value: QueryValue::From(ts),
                    });
                }
                "@to" => {
                    let ts = parse_timestamp(&token, tz_offset)
                        .ok_or(format!("invalid time format: {}", &token))?;
                    elements.push(QueryElement {
                        negated: false,
                        value: QueryValue::To(ts),
                    });
                }
                _ => {
                    elements.push(QueryElement {
                        negated,
                        value: QueryValue::KeyValue(key, token.to_string()),
                    });
                }
            }

            negated = false;
        } else {
            elements.push(QueryElement {
                negated,
                value: QueryValue::String(token),
            });
            negated = false;
        }
    }

    Ok(elements)
}

fn parse_token(input: &str) -> IResult<&str, String> {
    // Skip any leading whitespace.
    let (input, _) = multispace0(input)?;

    // Looking for a leading operator of ! or - for negation.
    let (input, op) = opt(alt((tag("!"), tag("-"))))(input)?;
    if let Some(op) = op {
        return Ok((input, op.to_string()));
    }

    if input.starts_with('"') {
        return Ok(parse_quoted_string(input));
    }

    let (input, token) = take_till(|c| c == ' ' || c == ':')(input)?;

    Ok((input, token.to_string()))
}

// Much like parse_token, but will consume ':' chars.
fn parse_value(input: &str) -> IResult<&str, String> {
    // Skip any leading whitespace.
    let (input, _) = multispace0(input)?;

    // Looking for a leading operator of ! or - for negation.
    let (input, op) = opt(alt((tag("!"), tag("-"))))(input)?;
    if let Some(op) = op {
        return Ok((input, op.to_string()));
    }

    if input.starts_with('"') {
        return Ok(parse_quoted_string(input));
    }

    let (input, token) = take_till(|c| c == ' ')(input)?;

    Ok((input, token.to_string()))
}

// Parse a quoted string.
//
// Returns a tuple where the first element is location after the
// quoted string, and the second element is the quoted string stripped
// of leading and trailing quotes, and any escape chars removed from
// inner quotes.
fn parse_quoted_string(input: &str) -> (&str, String) {
    assert!(input.starts_with('"'));
    let mut ptr = &input[1..];
    let mut string = String::new();

    let mut escaped = false;
    for c in ptr.chars() {
        ptr = &ptr[c.len_utf8()..];
        if escaped {
            string.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            break;
        } else {
            string.push(c);
        }
    }

    (ptr, string)
}

pub(crate) fn parse_timestamp(input: &str, tz_offset: Option<&str>) -> Option<DateTime<Utc>> {
    // First attempt to parse it as is.
    if let Ok(ts) = input.parse::<DateTime<Utc>>() {
        return Some(ts);
    }

    let default_tz = tz_offset.unwrap_or("Z");

    // Now attempt to match it and fill in the missing bits. Requires at least a year.
    //let re = r"^(\d{4})-?(\d{2})?-?(\d{2})?T?(\d{2})?:?(\d{2})?:?(\d{2})?\.?(\d+)?(([+\-]\d{4})|Z)?$";
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

        let fixed = format!(
            "{}-{}-{}T{}:{}:{}.{}{}",
            year, month, day, hour, minute, second, subs, offset,
        );

        // Try again.
        if let Ok(ts) = fixed.parse::<DateTime<Utc>>() {
            return Some(ts);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use chrono::SecondsFormat;

    use super::*;

    #[test]
    fn test_parse_timestamp() {
        let ts = parse_timestamp("2024-05-16T16:08:17.876423-0600", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T22:08:17.876Z"
        );

        let ts = parse_timestamp("2023-01-01T01:02:00.0+0000", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2023-01-01T01:02:00.000Z"
        );

        let ts = parse_timestamp("2024-05-16T16:08:17.876423+0600", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T10:08:17.876Z"
        );

        let ts = parse_timestamp("2024-05-16T16:08:17.876423Z", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T16:08:17.876Z"
        );

        let ts = parse_timestamp("2024", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-01-01T00:00:00.000Z"
        );

        let ts = parse_timestamp("2024-05", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-01T00:00:00.000Z"
        );

        let ts = parse_timestamp("2024-05-16", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T00:00:00.000Z"
        );

        let ts = parse_timestamp("2024-05-16T16", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T16:00:00.000Z"
        );

        let ts = parse_timestamp("2024-05-16T16:08", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T16:08:00.000Z"
        );

        let ts = parse_timestamp("2024-05-16T16:08:17", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T16:08:17.000Z"
        );

        let ts = parse_timestamp("2024-05-16T16:08:17.876", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T16:08:17.876Z"
        );

        let ts = parse_timestamp("2024-05-16T16:08:17Z", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T16:08:17.000Z"
        );

        let ts = parse_timestamp("2024-05-16+0000", None).unwrap();
        assert_eq!(
            ts.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-05-16T00:00:00.000Z"
        );
    }

    #[test]
    fn test_parse() {
        let elements = parse("foobar", None).unwrap();
        assert_eq!(elements.len(), 1);
        assert!(!elements[0].negated);
        assert_eq!(elements[0].value, QueryValue::String("foobar".to_string()));

        let elements = parse("foo:bar", None).unwrap();
        assert_eq!(elements.len(), 1);
        assert!(!elements[0].negated);
        assert_eq!(
            elements[0].value,
            QueryValue::KeyValue("foo".to_string(), "bar".to_string())
        );

        let elements = parse("-foo:bar", None).unwrap();
        assert_eq!(elements.len(), 1);
        assert!(elements[0].negated);
        assert_eq!(
            elements[0].value,
            QueryValue::KeyValue("foo".to_string(), "bar".to_string())
        );

        let elements = parse(r#""ET POLICY" -src_ip:10.10.10.10"#, None).unwrap();
        assert_eq!(elements.len(), 2);
        assert!(!elements[0].negated);
        assert_eq!(
            elements[0].value,
            QueryValue::String("ET POLICY".to_string())
        );
        assert!(elements[1].negated);
        assert_eq!(
            elements[1].value,
            QueryValue::KeyValue("src_ip".to_string(), "10.10.10.10".to_string())
        );

        let elements = parse(r#"src_ip:a.b.c.d @from:2024"#, None).unwrap();
        assert_eq!(elements.len(), 2);
        assert!(!elements[0].negated);
        assert_eq!(
            elements[0].value,
            QueryValue::KeyValue("src_ip".to_string(), "a.b.c.d".to_string())
        );
        assert!(!elements[1].negated);
        assert_eq!(
            elements[1].value,
            QueryValue::From(
                DateTime::parse_from_rfc3339("2024-01-01T00:00:00.000Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );

        let elements = parse(r#"dns -"et info" -"et \"dns"#, None).unwrap();
        assert_eq!(elements.len(), 3);

        assert!(!elements[0].negated);
        assert_eq!(elements[0].value, QueryValue::String("dns".to_string()));

        assert!(elements[1].negated);
        assert_eq!(elements[1].value, QueryValue::String("et info".to_string()));

        assert!(elements[2].negated);
        assert_eq!(
            elements[2].value,
            QueryValue::String("et \"dns".to_string())
        );

        let elements = parse(r#"@from:2024-05-16T09:48:44"#, Some("-0600")).unwrap();
        assert_eq!(elements.len(), 1);
        assert!(!elements[0].negated);
        assert_eq!(
            elements[0].value,
            QueryValue::From(
                DateTime::parse_from_rfc3339("2024-05-16T15:48:44.000Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
    }

    #[test]
    fn test_next_token() {
        let (rem, token) = parse_token("\"foobar\"asdf").unwrap();
        assert_eq!(rem, "asdf");
        assert_eq!(token, "foobar");

        // Space terminate value, not quoted.
        let (rem, token) = parse_token("foo bar").unwrap();
        assert_eq!(rem, " bar");
        assert_eq!(token, "foo");

        // ':' terminate value, not quoted.
        let (rem, token) = parse_token("foo:bar").unwrap();
        assert_eq!(rem, ":bar");
        assert_eq!(token, "foo");

        let (rem, token) = parse_token("foo::bar").unwrap();
        assert_eq!(rem, "::bar");
        assert_eq!(token, "foo");

        let (rem, token) = parse_token("").unwrap();
        assert_eq!(rem, "");
        assert_eq!(token, "");

        let (rem, token) = parse_token(":foo:bar").unwrap();
        assert_eq!(rem, ":foo:bar");
        assert_eq!(token, "");
    }

    #[test]
    fn test_parse_quoted() {
        let (n, s) = parse_quoted_string(r#""simple""#);
        assert_eq!(n, "");
        assert_eq!(s, "simple");

        let (n, s) = parse_quoted_string(r#""sim\"ple" and the rest"#);
        assert_eq!(n, " and the rest");
        assert_eq!(s, "sim\"ple");

        // Taken from Pawpatrules.
        let input = r#""🐾 - 🚨 Google Chrome / Chromium 🌐 Google Cast enabled (mDNS query observed)"; flow:to_server;"#;
        let (n, s) = parse_quoted_string(input);
        assert_eq!(n, "; flow:to_server;");
        assert_eq!(
            s,
            "🐾 - 🚨 Google Chrome / Chromium 🌐 Google Cast enabled (mDNS query observed)"
        );

        // No ending quote.
        let (n, s) = parse_quoted_string("\"testing; +asdf");
        assert_eq!(n, "");
        assert_eq!(s, "testing; +asdf");
    }
}
