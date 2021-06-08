// Copyright (c) 2020 Jason Ish
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

extern crate nom;

use std::io::BufRead;

use nom::{
    bytes::complete::tag,
    character::complete::multispace0,
    error::{ErrorKind, ParseError},
    multi::{many0, many0_count},
    sequence::{preceded, tuple},
    IResult,
};

static WHITESPACE: &str = " \t\r\n";

#[derive(Debug, PartialEq)]
enum InternalError<I> {
    NoListEnd,
    Nom(I, ErrorKind),
}

impl<I> ParseError<I> for InternalError<I> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        InternalError::Nom(input, kind)
    }

    fn append(_input: I, _kind: ErrorKind, other: Self) -> Self {
        other
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct RuleHeader {
    pub action: String,
    pub proto: String,
    pub src_addr: String,
    pub src_port: String,
    pub direction: String,
    pub dst_addr: String,
    pub dst_port: String,
}

#[derive(Debug, PartialEq)]
pub struct RuleOption {
    pub key: String,
    pub val: Option<String>,
}

#[derive(Debug)]
pub struct TokenizedRule {
    pub disabled: bool,
    pub header: RuleHeader,
    pub options: Vec<RuleOption>,
    pub original: String,
}

fn parse_header_token(input: &str) -> IResult<&str, &str, InternalError<&str>> {
    preceded(multispace0, nom::bytes::complete::is_not(WHITESPACE))(input)
}

/// Get the next rule option key (name).
///
/// This will return the next set of characters terminated by ; or : with all
/// leading whitespace removed.
fn get_option_key(input: &str) -> IResult<&str, &str, InternalError<&str>> {
    preceded(multispace0, nom::bytes::complete::is_not(";:"))(input)
}

/// Get the next option value.
///
/// This will return the next sequence of characters terminated by ';',
/// while removing any instances of an escaped ';'.
fn get_option_value(input: &str) -> IResult<&str, String, InternalError<&str>> {
    let mut output = Vec::new();
    let mut escaped = false;
    let mut end = 0;
    for (i, c) in input.chars().enumerate() {
        end = i;
        if c == '\\' {
            escaped = true;
        } else if escaped {
            if c == ';' {
                output.push(c);
            } else {
                output.push('\\');
                output.push(c);
            }
            escaped = false;
        } else if c == ';' {
            // Eat the ';'.
            end += 1;
            break;
        } else {
            output.push(c);
        }
    }
    let (_, rem) = input.split_at(end);
    Ok((rem, output.into_iter().collect()))
}

/// Parse a list as a token.
///
/// Returns the complete list as a single token, handling nested
/// lists, and spaces between listen items.
///
/// As this doesn't use nom, we make it look like a nom parser,
/// and use nom error types that are close enough.
fn parse_list_token(input: &str) -> IResult<&str, &str, InternalError<&str>> {
    // Peek into the input to make sure it looks like the start of a list.
    let input = preceded(
        multispace0,
        nom::combinator::peek(nom::bytes::complete::tag("[")),
    )(input)?
    .0;
    let mut in_list = 0;
    let mut end = 0;

    for (i, c) in input.chars().enumerate() {
        match c {
            '[' => in_list += 1,
            ']' => in_list -= 1,
            _ => {}
        }

        if in_list == 0 {
            end = i + 1;
            break;
        }
    }

    // If we got here and are still in the list, then we ran out of input.
    if in_list > 0 {
        return Err(nom::Err::Error(InternalError::NoListEnd));
    }

    let (list, rem) = input.split_at(end);
    Ok((rem, list))
}

fn parse_header(input: &str) -> IResult<&str, RuleHeader, InternalError<&str>> {
    let maybe_list = &nom::branch::alt((parse_list_token, parse_header_token));
    let (rem, (action, proto, src_addr, src_port, direction, dst_addr, dst_port)) = tuple((
        // Action.
        parse_header_token,
        // Proto.
        maybe_list,
        // Source address.
        maybe_list,
        // Source port.
        maybe_list,
        // Direction.
        parse_header_token,
        // Destination address.
        maybe_list,
        // Destination port.
        maybe_list,
    ))(input)?;
    Ok((
        rem,
        RuleHeader {
            action: String::from(action),
            proto: String::from(proto),
            src_addr: String::from(src_addr),
            src_port: String::from(src_port),
            direction: String::from(direction),
            dst_addr: String::from(dst_addr),
            dst_port: String::from(dst_port),
        },
    ))
}

fn parse_option(input: &str) -> IResult<&str, RuleOption, InternalError<&str>> {
    // Eat any leading space, then parse up to a : or a ;.
    let (input, _) = multispace0(input)?;
    let (input, key) = get_option_key(input)?;
    let (input, sep) = nom::character::complete::one_of(";:")(input)?;
    if sep == ';' {
        return Ok((
            input,
            RuleOption {
                key: String::from(key),
                val: None,
            },
        ));
    }

    // Drop any leading whitespace then parse up to ';', accounting for
    // escaped occurrences of ';'.
    let (input, _) = multispace0(input)?;
    let (input, val) = get_option_value(input)?;

    Ok((
        input,
        RuleOption {
            key: String::from(key),
            val: Some(strip_quotes(&val)),
        },
    ))
}

/// Remove quotes from a string, but preserve any escaped quotes.
fn strip_quotes(input: &str) -> String {
    let mut escaped = false;
    let mut out: Vec<char> = Vec::new();

    for c in input.chars() {
        match c {
            '"' if escaped => {
                out.push(c);
                escaped = false;
            }
            '"' => {}
            '\\' => {
                escaped = true;
            }
            _ => {
                if escaped {
                    out.push('\\');
                    escaped = false;
                }
                out.push(c);
            }
        }
    }

    out.iter().collect()
}

fn internal_parse_rule(input: &str) -> IResult<&str, TokenizedRule, InternalError<&str>> {
    let original = String::from(input);
    let (input, disabled) = preceded(multispace0, many0_count(tag("#")))(input)?;
    let (input, header) = parse_header(input)?;
    let (input, _) = preceded(multispace0, tag("("))(input)?;
    let (input, options) = many0(parse_option)(input)?;
    let (input, _) = preceded(multispace0, tag(")"))(input)?;

    Ok((
        input,
        TokenizedRule {
            disabled: disabled > 0,
            header,
            options,
            original,
        },
    ))
}

pub fn parse_rule(input: &str) -> anyhow::Result<TokenizedRule> {
    match internal_parse_rule(input) {
        Ok((_, rule)) => Ok(rule),
        Err(err) => Err(anyhow::anyhow!(err.to_string())),
    }
}

/// Read the next rule from a reader.
///
/// This will actually return any line it reads, but will join together
/// multiline rules into a single line rule.
///
/// Ok(None) will be returned on EOF, and an error will be returned on read
/// error.
pub fn read_next_rule(input: &mut dyn BufRead) -> Result<Option<String>, std::io::Error> {
    let mut line = String::new();
    loop {
        let mut tmp = String::new();
        let n = input.read_line(&mut tmp)?;
        if n == 0 {
            return Ok(None);
        }

        let tmp = tmp.trim();

        if !tmp.ends_with('\\') {
            line.push_str(tmp);
            break;
        }

        line.push_str(&tmp[..tmp.len() - 1]);
    }
    Ok(Some(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_token() {
        assert_eq!(parse_header_token("alert"), Ok(("", "alert")));
        assert_eq!(parse_header_token(" alert"), Ok(("", "alert")));
        assert_eq!(parse_header_token(" alert "), Ok((" ", "alert")));
        assert_eq!(parse_header_token("http_uri"), Ok(("", "http_uri")));
    }

    #[test]
    fn test_parse_quoted_string() {
        assert_eq!(
            strip_quotes(r#""some quoted \" string""#),
            r#"some quoted " string"#
        );
    }

    #[test]
    fn test_parse_list() {
        assert_eq!(parse_list_token("[1]"), Ok(("", "[1]")));
        assert_eq!(parse_list_token("[1,2,3]"), Ok(("", "[1,2,3]")));
        assert_eq!(parse_list_token(" [1,2,3]"), Ok(("", "[1,2,3]")));
        assert_eq!(
            parse_list_token(" [1,2,3,[a,b,c]]"),
            Ok(("", "[1,2,3,[a,b,c]]"))
        );

        assert!(parse_list_token("1,2,3]").is_err());
        assert!(parse_list_token("[1,2,3").is_err());

        assert!(parse_list_token("token").is_err());
    }

    #[test]
    fn test_parse_header() {
        let rule = parse_header("alert tcp any any -> any any");
        assert_eq!(
            rule,
            Ok((
                "",
                RuleHeader {
                    action: String::from("alert"),
                    proto: String::from("tcp"),
                    src_addr: String::from("any"),
                    src_port: String::from("any"),
                    direction: String::from("->"),
                    dst_addr: String::from("any"),
                    dst_port: String::from("any"),
                    ..Default::default()
                }
            ))
        );
    }

    #[test]
    fn test_parse_option_without_value() {
        assert_eq!(
            Ok((
                "",
                RuleOption {
                    key: String::from("http_uri"),
                    val: None,
                }
            )),
            parse_option("http_uri;")
        );
    }

    #[test]
    fn test_parse_option() {
        assert_eq!(
            parse_option("msg:value;"),
            Ok((
                "",
                RuleOption {
                    key: String::from("msg"),
                    val: Some("value".to_string()),
                }
            ))
        );

        assert_eq!(
            parse_option("msg:value with spaces;"),
            Ok((
                "",
                RuleOption {
                    key: String::from("msg"),
                    val: Some("value with spaces".to_string()),
                }
            ))
        );

        assert_eq!(
            parse_option("msg:terminated value with spaces;"),
            Ok((
                "",
                RuleOption {
                    key: String::from("msg"),
                    val: Some("terminated value with spaces".to_string()),
                }
            ))
        );

        assert_eq!(
            parse_option(r#"msg: an escaped \; terminant; next_option;"#),
            Ok((
                " next_option;",
                RuleOption {
                    key: String::from("msg"),
                    val: Some("an escaped ; terminant".to_string()),
                }
            ))
        );

        assert_eq!(
            parse_option(r#"msg: "A Quoted Message";"#),
            Ok((
                "",
                RuleOption {
                    key: String::from("msg"),
                    val: Some(r#"A Quoted Message"#.to_string()),
                }
            ))
        );

        assert_eq!(
            parse_option(r#"msg: "A Quoted Message with escaped \" quotes.";"#),
            Ok((
                "",
                RuleOption {
                    key: String::from("msg"),
                    val: Some(r#"A Quoted Message with escaped " quotes."#.to_string()),
                }
            ))
        );

        assert_eq!(
            parse_option(r#"pcre:"/^/index\.html/$/U";"#),
            Ok((
                "",
                RuleOption {
                    key: String::from("pcre"),
                    val: Some(r#"/^/index\.html/$/U"#.to_string()),
                }
            ))
        );
    }

    #[test]
    fn test_parse_rule() {
        let rule = parse_rule(r#"alert ip any any -> any any (msg:"Some Message with a \" Quote"; metadata: key, val; sid:1; rev:1;)"#).unwrap();
        assert_eq!(rule.disabled, false);
        assert_eq!(rule.header.action, "alert");

        let rule = parse_rule(r#"  alert ip any any -> any any (msg:"Some Message with a \" Quote"; metadata: key, val; sid:1; rev:1;)"#).unwrap();
        assert_eq!(rule.disabled, false);
        assert_eq!(rule.header.action, "alert");

        let rule = parse_rule(r#"#alert ip any any -> any any (msg:"Some Message with a \" Quote"; metadata: key, val; sid:1; rev:1;)"#).unwrap();
        assert_eq!(rule.disabled, true);
        assert_eq!(rule.header.action, "alert");

        let rule = parse_rule(r#"# alert ip any any -> any any (msg:"Some Message with a \" Quote"; metadata: key, val; sid:1; rev:1;)"#).unwrap();
        assert_eq!(rule.disabled, true);
        assert_eq!(rule.header.action, "alert");

        let rule = parse_rule(r#"###   alert ip any any -> any any (msg:"Some Message with a \" Quote"; metadata: key, val; sid:1; rev:1;)"#).unwrap();
        assert_eq!(rule.disabled, true);
        assert_eq!(rule.header.action, "alert");
    }

    #[test]
    fn test_tokenize_option_value() {
        assert_eq!(
            get_option_value("one; two"),
            Ok((" two", String::from("one")))
        );

        assert_eq!(
            get_option_value("one\\;; two"),
            Ok((" two", String::from("one;")))
        );

        let optval = r#""/^(?:[a-zA-Z0-9_%+])*(?:[\x2c\x22\x27\x28]|\x252[c278])/PRi";"#;
        let expected = r#""/^(?:[a-zA-Z0-9_%+])*(?:[\x2c\x22\x27\x28]|\x252[c278])/PRi""#;
        assert_eq!(get_option_value(optval), Ok(("", String::from(expected))));
    }

    #[test]
    fn test_parse_from_reader() {
        let input = r#"alert ip any any -> any any (\
            msg:"TEST RULE"; sid:1; rev:1;)"#;
        let mut reader = input.as_bytes();
        let next = read_next_rule(&mut reader).unwrap();
        assert_eq!(
            next,
            Some(r#"alert ip any any -> any any (msg:"TEST RULE"; sid:1; rev:1;)"#.to_string())
        );
        assert_eq!(read_next_rule(&mut reader).unwrap(), None);
    }
}
