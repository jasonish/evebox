// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

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
