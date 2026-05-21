// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::server::ServerContext;
use crate::server::main::SessionExtractor;
use crate::util::pcap;

use std::sync::Arc;

use axum::extract::{Extension, Form};
use axum::response::IntoResponse;
use serde::Deserialize;

use axum::http::HeaderMap;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};

use crate::error::AppError;

#[derive(Deserialize, Debug)]
pub(crate) struct PcapForm {
    pub what: String,
    pub event: String,
    pub filename: Option<String>,
}

pub(crate) async fn handler(
    Extension(_context): Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
    Form(form): Form<PcapForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut hmap = HeaderMap::new();
    hmap.insert(
        CONTENT_TYPE,
        "application/vnc.tcpdump.pcap".parse().unwrap(),
    );

    let event: serde_json::Value = serde_json::from_str(&form.event)
        .map_err(|err| AppError::BadRequest(format!("failed to decode event: {err}")))?;

    let pcap_buffer = pcap::eve_to_pcap(&form.what, &event)?;
    let filename = generate_filename(&event, form.filename.as_deref());

    let cs_hdr_value = format!("attachment; filename=\"{filename}.pcap\"");
    hmap.insert(CONTENT_DISPOSITION, cs_hdr_value.parse().unwrap());

    Ok((hmap, pcap_buffer))
}

/// Generate a filename for the PCAP file based on user input or the event.
fn generate_filename(event: &serde_json::Value, requested_filename: Option<&str>) -> String {
    if let Some(filename) = requested_filename.and_then(sanitize_filename) {
        return filename;
    }

    generate_default_filename(event)
}

fn generate_default_filename(event: &serde_json::Value) -> String {
    if let Some(sid) = &event["alert"]["signature_id"].as_u64() {
        if let Some(app_proto) = event["app_proto"].as_str().and_then(sanitize_filename) {
            return format!("{sid}-{app_proto}");
        }
        sid.to_string()
    } else {
        "event".to_string()
    }
}

fn sanitize_filename(filename: &str) -> Option<String> {
    let mut filename = filename.trim();
    if filename.len() >= 5 && filename[filename.len() - 5..].eq_ignore_ascii_case(".pcap") {
        filename = &filename[..filename.len() - 5];
    }

    let mut sanitized = String::new();
    let mut last_was_separator = false;
    for c in filename.chars() {
        let next = if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
            last_was_separator = false;
            Some(c)
        } else if c.is_whitespace() || c == '/' || c == '\\' || c.is_control() {
            if last_was_separator {
                None
            } else {
                last_was_separator = true;
                Some('-')
            }
        } else if last_was_separator {
            None
        } else {
            last_was_separator = true;
            Some('-')
        };

        if let Some(c) = next {
            sanitized.push(c);
        }
    }

    let sanitized = sanitized.trim_matches(['.', '-', '_']).to_string();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::generate_filename;

    #[test]
    fn generate_filename_uses_requested_filename_when_available() {
        let event = json!({
            "app_proto": "dns",
            "alert": {
                "signature_id": 12345,
            }
        });

        assert_eq!(
            "custom-capture",
            generate_filename(&event, Some("custom-capture"))
        );
    }

    #[test]
    fn generate_filename_strips_pcap_extension_from_requested_filename() {
        let event = json!({});

        assert_eq!(
            "custom-capture",
            generate_filename(&event, Some("custom-capture.pcap"))
        );
    }

    #[test]
    fn generate_filename_sanitizes_requested_filename() {
        let event = json!({});

        assert_eq!(
            "custom-capture",
            generate_filename(&event, Some("../custom\rcapture"))
        );
    }

    #[test]
    fn generate_filename_falls_back_when_requested_filename_is_blank() {
        let event = json!({
            "app_proto": "dns",
            "alert": {
                "signature_id": 12345,
            }
        });

        assert_eq!("12345-dns", generate_filename(&event, Some("  ")));
    }

    #[test]
    fn generate_filename_includes_sid_and_app_proto_when_available() {
        let event = json!({
            "app_proto": "dns",
            "alert": {
                "signature_id": 12345,
            }
        });

        assert_eq!("12345-dns", generate_filename(&event, None));
    }
}
