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

use crate::server::session::Session;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use warp::http::StatusCode;
use warp::Reply;

pub enum Response {
    // Successful responses.
    Ok,
    Json(serde_json::Value),
    StatusCode(warp::http::StatusCode),

    // Error responses.
    QueryStringParseError,
    NotFound,
    InternalError(String),
    Unimplemented,
    TimestampParseError(String),
    Unauthorized,
}

impl warp::Reply for Response {
    fn into_response(self) -> warp::reply::Response {
        match self {
            Self::Json(json) => {
                warp::reply::with_status(warp::reply::json(&json), StatusCode::OK).into_response()
            }
            Self::StatusCode(status) => status.into_response(),
            Self::Ok => StatusCode::OK.into_response(),
            Self::QueryStringParseError => {
                let code = StatusCode::BAD_REQUEST;
                let resp = json!({
                    "code": code.as_u16(),
                    "error": "failed to parse query string",
                });
                warp::reply::with_status(warp::reply::json(&resp), code).into_response()
            }
            Self::Unauthorized => Self::build_error_response(
                StatusCode::UNAUTHORIZED,
                &StatusCode::UNAUTHORIZED.to_string(),
            ),
            Self::NotFound => Self::build_error_response(
                StatusCode::NOT_FOUND,
                &StatusCode::NOT_FOUND.to_string(),
            ),
            Self::InternalError(error) => {
                Self::build_error_response(StatusCode::INTERNAL_SERVER_ERROR, &error)
            }
            Self::Unimplemented => Self::build_error_response(
                StatusCode::NOT_IMPLEMENTED,
                &StatusCode::NOT_IMPLEMENTED.to_string(),
            ),
            Self::TimestampParseError(ts) => Self::build_error_response(
                StatusCode::BAD_REQUEST,
                &format!("failed to parse timestamp: {}", ts),
            ),
        }
    }
}

impl Response {
    pub fn build_error_response(code: StatusCode, error: &str) -> warp::reply::Response {
        let body = ErrorResponse {
            code: code.as_u16(),
            error: error.to_string(),
        };
        warp::reply::with_status(warp::reply::json(&body), code).into_response()
    }

    pub fn with_session(self, session: Arc<Session>) -> warp::reply::Response {
        warp::reply::with_header(
            self.into_response(),
            "x-evebox-session-id",
            &session.session_id,
        )
        .into_response()
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    pub code: u16,
    pub error: String,
}
