// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

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

pub fn parse(mut input: &str) -> IResult<&str, Vec<Element>> {
    let mut tokens = vec![];
    loop {
        if input.is_empty() {
            break;
        }
        let (next, _) = multispace0(input)?;
        let (next, token) = alt((parse_quoted, parse_string))(next)?;
        if next.starts_with(':') {
            let (next, value) = preceded(tag(":"), alt((parse_quoted, parse_string)))(next)?;
            match token {
                "@ip" => tokens.push(Element::Ip(value.to_string())),
                _ => tokens.push(Element::KeyVal(token.to_string(), value.to_string())),
            }
            input = next;
        } else {
            tokens.push(Element::String(token.to_string()));
            input = next;
        }
    }
    Ok(("", tokens))
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_query_string() {
        let (_rem, parsed) = parse(r#""ET DROP""#).unwrap();
        assert_eq!(parsed, vec![Element::String("ET DROP".to_string())]);

        let (_rem, parsed) = parse(r#"flow_id:1"#).unwrap();
        assert_eq!(
            parsed,
            vec![Element::KeyVal("flow_id".to_string(), "1".to_string())]
        );

        let (_rem, parsed) = parse(r#"alert.signature:"ET DROP Spamhaus""#).unwrap();
        assert_eq!(
            parsed,
            vec![Element::KeyVal(
                "alert.signature".to_string(),
                "ET DROP Spamhaus".to_string()
            )]
        );

        let (_rem, parsed) = parse(r#"flow:1 alert.signature:"ET DROP Spamhaus""#).unwrap();
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
            parse(r#"flow:1 alert.signature:"ET DROP Spamhaus" "SOME Quoted String""#).unwrap();
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
            parse(r#"flow:1 alert.signature:"ET DROP Spamhaus" bad'ly"formatted"#).unwrap();
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

        let (_rem, parsed) = parse(r#""no-end-quote"#).unwrap();
        assert_eq!(
            parsed,
            vec![Element::String(r#""no-end-quote"#.to_string())]
        );

        let (_, parsed) = parse("token asdf").unwrap();
        assert_eq!(
            &parsed,
            &[
                Element::String("token".to_string()),
                Element::String("asdf".to_string())
            ]
        );

        let (_, parsed) = parse("alert.signature:\"WPAD\" 10.16.1.1").unwrap();
        let first = &parsed[0];
        assert_eq!(
            first,
            &Element::KeyVal("alert.signature".to_string(), "WPAD".to_string())
        );
    }

    #[test]
    fn test_parse_token() {
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
}
