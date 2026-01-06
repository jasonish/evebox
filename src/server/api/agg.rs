// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::util::parse_duration;
use crate::error::AppError;
use crate::eventrepo::EventRepo;
use crate::prelude::*;
use crate::queryparser;
use crate::queryparser::{QueryElement, QueryValue};
use crate::server::{ServerContext, main::SessionExtractor};
use axum::Extension;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::{Form, Json, extract::State, response::IntoResponse};
use futures::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;
use std::{ops::Sub, sync::Arc};

#[derive(Debug, Deserialize)]
pub(crate) struct AggParams {
    /// Field name to group and return the counts for.
    pub(crate) field: String,
    /// Humanized time range string.
    #[serde(default = "default_time_range")]
    pub(crate) time_range: String,
    /// Number of results to return.
    #[serde(default = "default_size")]
    pub(crate) size: usize,
    /// Sort order, desc or asc.
    #[serde(default = "default_order")]
    pub(crate) order: String,
    /// Optional query string.
    pub(crate) q: Option<String>,
    pub(crate) tz_offset: Option<String>,
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

pub(crate) async fn agg_sse(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(form): Form<AggParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, Infallible>>();

    // First parse the query string.
    let default_tz_offset = form.tz_offset.as_deref();
    let mut query_string = form
        .q
        .clone()
        .map(|qs| queryparser::parse(&qs, default_tz_offset))
        .transpose()
        .unwrap()
        .unwrap_or_default();

    let min_timestamp = parse_duration(&form.time_range)
        .map(|d| chrono::Utc::now().sub(d))
        .map_err(|err| AppError::BadRequest(format!("time_range: {err}")))
        .unwrap();
    query_string.push(QueryElement {
        negated: false,
        value: QueryValue::From(min_timestamp.into()),
    });

    // For Elastic and Postgres, use the fast GROUP BY aggregation path.
    // These databases handle aggregation efficiently server-side.
    match &context.datastore {
        EventRepo::Elastic(ds) => {
            let result = ds
                .agg(&form.field, form.size, &form.order, query_string.clone())
                .await?;
            let response = json!({
                "rows": result,
                "done": true,
            });
            let event = Event::default()
                .json_data(response)
                .map_err(|err| AppError::StringError(format!("{err:?}")))?;
            let _ = tx.send(Ok(event));

            let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
            return Ok(Sse::new(stream).keep_alive(
                axum::response::sse::KeepAlive::new()
                    .interval(Duration::from_secs(1))
                    .text("keep-alive-text"),
            ));
        }
        EventRepo::Postgres(ds) => {
            let result = ds
                .agg(&form.field, form.size, &form.order, query_string.clone())
                .await?;
            let response = json!({
                "rows": result,
                "done": true,
            });
            let event = Event::default()
                .json_data(response)
                .map_err(|err| AppError::StringError(format!("{err:?}")))?;
            let _ = tx.send(Ok(event));

            let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
            return Ok(Sse::new(stream).keep_alive(
                axum::response::sse::KeepAlive::new()
                    .interval(Duration::from_secs(1))
                    .text("keep-alive-text"),
            ));
        }
        EventRepo::SQLite(_) => {
            // SQLite uses streaming for progressive updates
        }
    }

    // SQLite streaming path - provides progressive updates for potentially slow queries
    tokio::spawn(async move {
        if let EventRepo::SQLite(ds) = &context.datastore {
            let (aggtx, mut aggrx) = tokio::sync::mpsc::unbounded_channel();

            let tx0 = tx.clone();
            let field = form.field.clone();
            tokio::spawn(async move {
                while let Some(result) = aggrx.recv().await {
                    if let Ok(event) = Event::default().json_data(result) {
                        if tx0.send(Ok(event)).is_err() {
                            debug!("Client disappeared, terminating SSE agg ({})", field);
                            return;
                        }
                    }
                }
            });

            if let Err(err) = ds
                .agg_stream(
                    &form.field,
                    form.size,
                    &form.order,
                    query_string,
                    Some(aggtx),
                )
                .await
            {
                // Log the error server-side
                error!("SSE agg stream error (SQLite): {err}");
                // Sanitize error message - SSE comments cannot contain newlines
                let err_msg = format!("error: {err}").replace(['\n', '\r'], " ");
                let event = Event::default().comment(err_msg);
                let _ = tx.send(Ok(event));
            }

            let event = Event::default().comment("done");
            let _ = tx.send(Ok(event));
        }
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    ))
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
        crate::eventrepo::EventRepo::Postgres(ds) => {
            let results = ds.get_event_types(query_string).await?;
            Ok(Json(results))
        }
    }
}
