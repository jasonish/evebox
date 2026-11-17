// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::time::Duration;
use std::time::UNIX_EPOCH;

/// Parse a string representing a duration.
///
/// The shared duration parser with special handlers for "", "all" and "*",
/// which return the duration since the Unix epoch.
pub(crate) fn parse_duration(duration: &str) -> Result<Duration> {
    match duration {
        "" | "all" | "*" => Ok(UNIX_EPOCH.elapsed()?),
        _ => crate::duration::parse(duration),
    }
}

/// The structured error body shared by the retrieval APIs (packet
/// capture and extracted files): `{"error": {"code": ..., "message":
/// ...}}` with a status. The webapp maps `code` to its own wording and
/// falls back to `message`.
pub(crate) fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    let body = json!({ "error": { "code": code, "message": message } });
    (status, Json(body)).into_response()
}

/// The present, non-blank value of an optional string field. Blank
/// (empty or whitespace) reads as absent.
pub(crate) fn present(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_time_sentinels_are_preserved() {
        for input in ["", "all", "*"] {
            let before = UNIX_EPOCH.elapsed().unwrap();
            let duration = parse_duration(input).unwrap();
            let after = UNIX_EPOCH.elapsed().unwrap();
            assert!(duration >= before && duration <= after, "{input:?}");
        }
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(5400));
        assert!(parse_duration("1 month").is_err());
    }
}
