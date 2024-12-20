// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::num::ParseIntError;

use thiserror::Error;

use crate::queryparser::QueryStringParseError;

#[derive(Error, Debug)]
pub(crate) enum AppError {
    #[error("{0}")]
    StringError(String),

    /// Essentially a string error without the string.
    #[error("internal server error")]
    InternalServerError,

    /// An error resulting from bad input data, such as an invalid
    /// timestamp. For API requests this will result in
    /// StatusCode::BAD_REQUEST.
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unimplemented")]
    Unimplemented,

    #[error("elasticsearch error: {0}")]
    ElasticSearchError(String),

    #[error("{0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("serde: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("event not found")]
    EventNotFound,

    #[error("failed to parse integer")]
    ParseIntError(#[from] ParseIntError),

    #[error("time parser error: {0}")]
    DateTimeParse(#[from] crate::datetime::ParseError),

    #[error("sqlx: {0}")]
    SqlxError(#[from] sqlx::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::StringError(value.to_string())
    }
}

impl From<QueryStringParseError> for AppError {
    fn from(value: QueryStringParseError) -> Self {
        Self::BadRequest(format!("failed to parse query string: {}", value))
    }
}

impl From<Box<dyn std::error::Error + std::marker::Send + Sync>> for AppError {
    fn from(value: Box<dyn std::error::Error + std::marker::Send + Sync>) -> Self {
        Self::StringError(value.to_string())
    }
}
