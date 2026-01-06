// SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

/// Returns a flattened string of all values found in a JSON object.
///
/// Simple values like null and bools are not returned. Also known
/// non-printable values (like base64 data) is not included. This is
/// used as the input to the full text search engine.
///
/// Special handling:
/// - `payload_printable` and `http.http_response_body_printable`: These contain
///   decoded binary data that often produces noisy, non-meaningful tokens.
///   Only alphanumeric words of 2+ characters are extracted.
pub fn extract_values(input: &serde_json::Value) -> String {
    fn push_word(output: &mut String, bytes: &[u8]) {
        if bytes.len() < 2 {
            return;
        }
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(&String::from_utf8_lossy(bytes));
    }

    fn extract_printable_words(input: &str, output: &mut String) {
        let mut start: Option<usize> = None;
        for (idx, b) in input.as_bytes().iter().copied().enumerate() {
            let is_alnum = matches!(b, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z');
            if is_alnum {
                if start.is_none() {
                    start = Some(idx);
                }
            } else if let Some(s) = start.take() {
                push_word(output, &input.as_bytes()[s..idx]);
            }
        }
        if let Some(s) = start.take() {
            push_word(output, &input.as_bytes()[s..]);
        }
    }

    fn inner<'a>(input: &'a serde_json::Value, output: &mut String, path: &mut Vec<&'a str>) {
        match input {
            serde_json::Value::Null | serde_json::Value::Bool(_) => {
                // Intentionally empty.
            }
            serde_json::Value::Number(n) => {
                if !output.is_empty() {
                    output.push(' ');
                }
                output.push_str(&n.to_string());
            }
            serde_json::Value::String(s) => {
                // Printable fields contain decoded binary that produces noisy tokens,
                // so extract only alphanumeric words of 2+ characters.
                let is_printable_field = path == &["payload_printable"]
                    || path == &["http", "http_response_body_printable"];
                if is_printable_field {
                    extract_printable_words(s, output);
                } else {
                    if !output.is_empty() {
                        output.push(' ');
                    }
                    output.push_str(s);
                }
            }
            serde_json::Value::Array(a) => {
                for e in a {
                    inner(e, output, path);
                }
            }
            serde_json::Value::Object(o) => {
                for (k, v) in o {
                    match k.as_ref() {
                        // Skip base64 encoded fields.
                        "packet" | "payload" => {}
                        // Skip alert rule metadata.
                        "rule" => {}
                        _ => {
                            path.push(k);
                            // Skip base64 encoded field.
                            if path != &["http", "http_response_body"] {
                                inner(v, output, path);
                            }
                            path.pop();
                        }
                    }
                }
            }
        }
    }

    let mut flattened = String::new();
    let mut path = Vec::with_capacity(8);
    inner(input, &mut flattened, &mut path);
    flattened
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_values_basic() {
        let event = json!({
            "timestamp": "2024-01-01T00:00:00.000000+0000",
            "src_ip": "192.168.1.1",
            "dest_ip": "10.0.0.1",
            "src_port": 12345,
            "dest_port": 80,
        });
        let result = extract_values(&event);
        assert!(result.contains("192.168.1.1"));
        assert!(result.contains("10.0.0.1"));
        assert!(result.contains("12345"));
        assert!(result.contains("80"));
    }

    #[test]
    fn test_extract_values_skips_packet() {
        let event = json!({
            "src_ip": "192.168.1.1",
            "packet": "SGVsbG8gV29ybGQ=",
        });
        let result = extract_values(&event);
        assert!(result.contains("192.168.1.1"));
        assert!(!result.contains("SGVsbG8"));
    }

    #[test]
    fn test_extract_values_skips_payload() {
        let event = json!({
            "src_ip": "192.168.1.1",
            "payload": "SGVsbG8gV29ybGQ=",
        });
        let result = extract_values(&event);
        assert!(result.contains("192.168.1.1"));
        assert!(!result.contains("SGVsbG8"));
    }

    #[test]
    fn test_extract_values_skips_http_response_body() {
        let event = json!({
            "src_ip": "192.168.1.1",
            "http": {
                "hostname": "example.com",
                "http_response_body": "SGVsbG8gV29ybGQ="
            }
        });
        let result = extract_values(&event);
        assert!(result.contains("192.168.1.1"));
        assert!(result.contains("example.com"));
        assert!(!result.contains("SGVsbG8"));
    }

    #[test]
    fn test_extract_values_printable_field() {
        let event = json!({
            "src_ip": "192.168.1.1",
            "payload_printable": "GET /test HTTP/1.1\r\n"
        });
        let result = extract_values(&event);
        assert!(result.contains("192.168.1.1"));
        assert!(result.contains("GET"));
        assert!(result.contains("test"));
        assert!(result.contains("HTTP"));
        // Single characters and punctuation should be filtered out
        assert!(!result.contains("\r\n"));
    }

    #[test]
    fn test_extract_values_skips_rule() {
        let event = json!({
            "src_ip": "192.168.1.1",
            "alert": {
                "signature": "Test Alert",
                "rule": "alert tcp any any -> any any (msg:\"test\";)"
            }
        });
        let result = extract_values(&event);
        assert!(result.contains("192.168.1.1"));
        assert!(result.contains("Test Alert"));
        assert!(!result.contains("alert tcp"));
    }
}
