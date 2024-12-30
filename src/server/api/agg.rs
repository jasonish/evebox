// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::util::parse_duration;
use crate::error::AppError;
use crate::queryparser;
use crate::queryparser::{QueryElement, QueryValue};
use crate::server::{main::SessionExtractor, ServerContext};
use axum::{extract::State, response::IntoResponse, Form, Json};
use serde::Deserialize;
use std::{ops::Sub, sync::Arc};

#[derive(Debug, Deserialize)]
pub(crate) struct AggParams {
    /// Field name to group and return the counts for.
    field: String,
    /// Humanized time range string.
    #[serde(default = "default_time_range")]
    time_range: String,
    /// Number of results to return.
    #[serde(default = "default_size")]
    size: usize,
    /// Sort order, desc or asc.
    #[serde(default = "default_order")]
    order: String,
    /// Optional query string.
    q: Option<String>,
    tz_offset: Option<String>,
}

const fn default_size() -> usize {
    10
}

fn default_time_range() -> String {
    "24h".to_string()
}

fn default_order() -> String {
    "desc".to_string()
}

pub(crate) async fn agg(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<AggParams>,
) -> Result<impl IntoResponse, AppError> {
    // First parse the query string.
    let default_tz_offset = form.tz_offset.as_deref();
    let mut query_string = form
        .q
        .clone()
        .map(|qs| queryparser::parse(&qs, default_tz_offset))
        .transpose()?
        .unwrap_or_default();

    let min_timestamp = parse_duration(&form.time_range)
        .map(|d| chrono::Utc::now().sub(d))
        .map_err(|err| AppError::BadRequest(format!("time_range: {err}")))?;
    query_string.push(QueryElement {
        negated: false,
        value: QueryValue::From(min_timestamp.into()),
    });

    let results = context
        .datastore
        .agg(&form.field, form.size, &form.order, query_string)
        .await?;
    #[rustfmt::skip]
    let response = json!({
	"rows": results,
    });
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
pub(crate) struct EventTypesParams {
    /// Humanized time range string.
    #[serde(default = "default_time_range")]
    time_range: String,
}

pub(crate) async fn event_types(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<EventTypesParams>,
) -> Result<impl IntoResponse, AppError> {
    let mut query_string = vec![];
    let min_timestamp = parse_duration(&form.time_range)
        .map(|d| chrono::Utc::now().sub(d))
        .map_err(|err| AppError::BadRequest(format!("time_range: {err}")))?;
    query_string.push(QueryElement {
        negated: false,
        value: QueryValue::From(min_timestamp.into()),
    });

    match &context.datastore {
        crate::eventrepo::EventRepo::Elastic(ds) => {
            let results = ds.get_event_types().await?;
            Ok(Json(results))
        }
        crate::eventrepo::EventRepo::SQLite(ds) => {
            let results = ds.get_event_types(query_string).await?;
            Ok(Json(results))
        }
    }
}
