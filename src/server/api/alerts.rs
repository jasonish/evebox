// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{elastic, prelude::*};

use std::sync::Arc;

use axum::{Extension, Json, response::IntoResponse};
use axum_extra::extract::Form;

use super::{DateTime, GenericQuery, ServerContext, SessionExtractor, parse_then_from_duration};

pub(crate) async fn alerts(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(query): Form<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let mut options = elastic::AlertQueryOptions {
        query_string: query.query_string,
        sensor: query.sensor,
        timeout: query.timeout,
        ..elastic::AlertQueryOptions::default()
    };

    if let Some(tags) = query.tags {
        if !tags.is_empty() {
            let tags: Vec<String> = tags.split(',').map(|s| s.to_string()).collect();
            options.tags = tags;
        }
    }

    if let Some(time_range) = query.time_range {
        if !time_range.is_empty() {
            let now = DateTime::now();
            match parse_then_from_duration(&now, &time_range) {
                None => {
                    error!("Failed to parse time_range: {}", time_range);
                }
                Some(then) => {
                    options.timestamp_gte = Some(then);
                }
            }
        }
    }

    if let Some(_ts) = query.min_timestamp {
        error!("alert_query: min_timestamp query argument not implemented");
    }

    if let Some(_ts) = query.max_timestamp {
        error!("alert_query: max_timeestamp query argument not implemented");
    }

    Ok(Json(
        context
            .datastore
            .alerts(options, context.auto_archive.clone())
            .await?,
    )
    .into_response())
}
