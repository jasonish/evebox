// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::{util::parse_duration, ApiError};
use crate::prelude::*;
use crate::{
    datastore::Datastore,
    server::{main::SessionExtractor, ServerContext},
};
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
}

pub(crate) async fn group_by(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<GroupByParams>,
) -> Result<impl IntoResponse, ApiError> {
    let duration = parse_duration(&form.time_range)
        .map_err(|err| ApiError::bad_request(format!("time_range: {err}")))?;
    let min_timestamp = time::OffsetDateTime::now_utc().sub(duration);

    match &context.datastore {
        Datastore::Elastic(_) => Err(ApiError::Unimplemented),
        Datastore::SQLite(ds) => {
            let results = ds
                .group_by(&form.field, min_timestamp, form.size, &form.order)
                .await
                .map_err(|err| {
                    error!("Datastore group by failed: {err}");
                    ApiError::InternalServerError
                })?;
            #[rustfmt::skip]
            let response = json!({
		"rows": results,
		"debug": {
                    "parsed_timerange": duration,
		}
            });
            Ok(Json(response))
        }
    }
}
