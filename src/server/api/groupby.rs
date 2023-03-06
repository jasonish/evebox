// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::{util::parse_duration, ApiError};
use crate::querystring::{Element, QueryString};
use crate::server::{main::SessionExtractor, ServerContext};
use crate::{prelude::*, querystring};
use axum::{extract::State, response::IntoResponse, Form, Json};
use serde::Deserialize;
use std::{ops::Sub, sync::Arc};

const fn default_size() -> usize {
    10
}

fn default_time_range() -> String {
    "24h".to_string()
}

fn default_order() -> String {
    "desc".to_string()
}

#[derive(Debug, Deserialize)]
pub(crate) struct GroupByParams {
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
}

pub(crate) async fn group_by(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<GroupByParams>,
) -> Result<impl IntoResponse, ApiError> {
    // First parse the query string.
    let mut q = form
        .q
        .as_ref()
        .map(|q| querystring::parse(q, None))
        .unwrap_or_else(|| Ok(vec![]))
        .map_err(|err| ApiError::bad_request(format!("q: {err}")))?;

    // Only apply the time range if the query string does not contain
    // an low timestamp.
    if !q.has_earliest() {
        let min_timestamp = parse_duration(&form.time_range)
            .map(|v| time::OffsetDateTime::now_utc().sub(v))
            .map_err(|err| ApiError::bad_request(format!("time_range: {err}")))?;
        q.push(Element::EarliestTimestamp(min_timestamp));
    }

    let results = context
        .datastore
        .group_by(&form.field, form.size, &form.order, q)
        .await
        .map_err(|err| {
            error!("Event repo group by failed: {err}");
            ApiError::InternalServerError
        })?;
    #[rustfmt::skip]
    let response = json!({
	"rows": results,
    });
    Ok(Json(response))
}
