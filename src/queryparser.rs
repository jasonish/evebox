// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use nom::{
    branch::alt,
    bytes::complete::{tag, take_till},
    character::complete::multispace0,
    combinator::opt,
    IResult,
};

use crate::datetime;

#[derive(Debug, Clone)]
pub(crate) struct QueryStringParseError(String);

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

impl From<datetime::ParseError> for QueryStringParseError {
    fn from(value: datetime::ParseError) -> Self {
        Self(format!("bad time format: {}", value))
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
    pub fn first_from(&self) -> Option<datetime::DateTime> {
        for element in &self.elements {
            if let QueryValue::From(ts) = &element.value {
                return Some(ts.clone());
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QueryValue {
    String(String),
    KeyValue(String, String),
    From(datetime::DateTime),
    To(datetime::DateTime),
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
                    let ts = datetime::parse(&token, tz_offset)?;
                    elements.push(QueryElement {
                        negated: false,
                        value: QueryValue::From(ts),
                    });
                }
                "@to" => {
                    let ts = datetime::parse(&token, tz_offset)?;
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
            let token = token.trim();
            if !token.is_empty() {
                elements.push(QueryElement {
                    negated,
                    value: QueryValue::String(token.to_string()),
                });
            }
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

#[cfg(test)]
mod tests {
    use super::*;

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
