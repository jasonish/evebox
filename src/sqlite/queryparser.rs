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

pub fn parse_query_string(input: &str) -> (Option<String>, String, &str) {
    let (rem, token) = next_token(input);
    if !rem.is_empty() && rem.starts_with(':') {
        let (rem, mut val) = next_token(&rem[1..]);
        if val.starts_with('"') && val.ends_with('"') {
            val = val[1..val.len() - 1].to_string();
        }
        return (Some(token), val, rem);
    }
    (None, token, rem)
}

fn next_token(input: &str) -> (&str, String) {
    let mut output = Vec::new();
    let mut escaped = false;
    let mut end = 0;
    let mut inquotes = false;
    let mut trim_terminator = true;
    for (i, c) in input.chars().enumerate() {
        end = i;
        if c == '\\' {
            // Disable escaping for now, not sure if its needed.
            //escaped = true;
        } else if escaped {
            if c == ';' {
                output.push(c);
            } else {
                output.push('\\');
                output.push(c);
            }
            escaped = false;
        } else if c == ' ' {
            if !inquotes {
                break;
            } else {
                output.push(c);
            }
        } else if c == '"' {
            if inquotes {
                inquotes = false;
            } else {
                inquotes = true;
            }
            output.push(c);
        } else if c == ':' {
            if inquotes {
                output.push(c);
            } else {
                trim_terminator = false;
                break;
            }
        } else {
            output.push(c);
        }
    }
    if trim_terminator {
        end += 1
    }
    let (_, rem) = input.split_at(std::cmp::min(end, input.len()));
    (rem, output.into_iter().collect())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_next_token() -> Result<(), Box<dyn std::error::Error>> {
        let token = next_token("token asdf");
        assert_eq!(token, ("asdf", "token".to_string()));

        let token = next_token("\"quoted string\"");
        assert_eq!(token, ("", "\"quoted string\"".to_string()));

        let token = next_token("key:val");
        assert_eq!(token, (":val", "key".to_string()));

        let (rem, token) = next_token("\"quoted key\":\"quoted value\" and some other text");
        assert_eq!(token, r#""quoted key""#.to_string());
        let (rem, token) = next_token(&rem[1..]);
        assert_eq!(token, r#""quoted value""#.to_string());
        assert_eq!(rem, "and some other text");

        Ok(())
    }

    #[test]
    fn test_parse_query_string() -> Result<(), Box<dyn std::error::Error>> {
        let qs = "alert.signature:\"WPAD\" 10.16.1.1";

        let (key, val, rem) = parse_query_string(qs);
        assert_eq!(key, Some("alert.signature".to_string()));
        assert_eq!(val, "\"WPAD\"".to_string());
        assert_eq!(rem, "10.16.1.1".to_string());

        let (key, val, rem) = parse_query_string(rem);
        assert_eq!(key, None);
        assert_eq!(val, "10.16.1.1".to_string());
        assert_eq!(rem, "");

        let (key, val, rem) = parse_query_string(rem);
        assert_eq!(key, None);
        assert_eq!(val, "".to_string());
        assert_eq!(rem, "");

        let (key, val, rem) = parse_query_string(rem);
        assert_eq!(key, None);
        assert_eq!(val, "".to_string());
        assert_eq!(rem, "");

        Ok(())
    }

    fn parse_query_string(input: &str) -> (Option<String>, String, &str) {
        let (rem, token) = next_token(input);
        if !rem.is_empty() && rem.starts_with(":") {
            let (rem, val) = next_token(&rem[1..]);
            return (Some(token), val, rem);
        }
        (None, token, rem)
    }

    fn next_token(input: &str) -> (&str, String) {
        let mut output = Vec::new();
        let mut escaped = false;
        let mut end = 0;
        let mut inquotes = false;
        let mut trim_terminator = true;
        for (i, c) in input.chars().enumerate() {
            end = i;
            if c == '\\' {
                // Disable escaping for now, not sure if its needed.
                //escaped = true;
            } else if escaped {
                if c == ';' {
                    output.push(c);
                } else {
                    output.push('\\');
                    output.push(c);
                }
                escaped = false;
            } else if c == ' ' {
                if !inquotes {
                    break;
                } else {
                    output.push(c);
                }
            } else if c == '"' {
                if inquotes {
                    inquotes = false;
                } else {
                    inquotes = true;
                }
                output.push(c);
            } else if c == ':' {
                if inquotes {
                    output.push(c);
                } else {
                    trim_terminator = false;
                    break;
                }
            } else {
                output.push(c);
            }
        }
        if trim_terminator {
            end += 1
        }
        let (_, rem) = input.split_at(std::cmp::min(end, input.len()));
        (rem, output.into_iter().collect())
    }
}
