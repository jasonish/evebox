// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::datastore;
use crate::datastore::Datastore;
use crate::prelude::*;
use crate::server::api::ApiError;
use crate::server::main::SessionExtractor;
use crate::server::ServerContext;
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;
use time::Duration;

pub(crate) fn router() -> Router<Arc<ServerContext>> {
    Router::new()
        .route("/agg/diff", get(agg_differential))
        .route("/agg", get(agg))
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StatsAggQuery {
    field: String,
    sensor_name: Option<String>,
    time_range: Option<i64>,
}

impl StatsAggQuery {
    /// Return the time range as a time::Duration as specified in the query.
    ///
    /// 0 means all time available
    /// None will default to 24 hours.
    fn duration(&self) -> time::Duration {
        self.time_range
            .map(|range| {
                if range == 0 {
                    time::Duration::MAX
                } else {
                    time::Duration::seconds(range)
                }
            })
            .unwrap_or_else(|| time::Duration::hours(24))
    }

    fn start_time(&self) -> anyhow::Result<time::OffsetDateTime> {
        let start_time = if let Some(time_range) = self.time_range {
            if time_range == 0 {
                time::OffsetDateTime::UNIX_EPOCH
            } else {
                time::OffsetDateTime::now_utc()
                    .checked_sub(time::Duration::seconds(time_range))
                    .ok_or_else(|| anyhow::anyhow!("overflow"))?
            }
        } else {
            time::OffsetDateTime::now_utc()
                .checked_sub(time::Duration::hours(24))
                .ok_or_else(|| anyhow::anyhow!("overflow"))?
        };
        Ok(start_time)
    }
}

fn bucket_interval(duration: time::Duration) -> time::Duration {
    let result = if duration > time::Duration::days(7) {
        time::Duration::minutes(5)
    } else if duration <= Duration::minutes(1) {
        time::Duration::seconds(5)
    } else {
        time::Duration::minutes(1)
    };
    debug!(
        "Converted duration of {:?} to bucket interval of {:?}",
        duration, result
    );
    result
}

async fn agg(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let duration = form.duration();
    let start_time = form.start_time().unwrap();
    let params = datastore::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        duration,
        interval: bucket_interval(duration),
        start_time,
    };

    match context.datastore.stats_agg(&params).await {
        Ok(response) => Ok(Json(response)),
        Err(err) => {
            error!(
                "Stats agg differential query failed: params={:?}, error={:?}",
                &params, err
            );
            Err(ApiError::InternalServerError)
        }
    }
}

async fn agg_differential(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let duration = form.duration();
    let start_time = form.start_time().unwrap();
    let params = datastore::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        duration,
        interval: bucket_interval(duration),
        start_time,
    };

    match context.datastore.stats_agg_diff(&params).await {
        Ok(response) => Ok(Json(response)),
        Err(err) => {
            error!(
                "Stats agg differential query failed: params={:?}, error={:?}",
                &params, err
            );
            Err(ApiError::InternalServerError)
        }
    }
}

// Doesn't really belong in this module.
pub(crate) async fn get_sensor_names(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
) -> Result<impl IntoResponse, ApiError> {
    if let Datastore::Elastic(elastic) = &context.datastore {
        let sensors = elastic.get_sensors().await.map_err(|err| {
            error!("Failed to get sensors: {:?}", err);
            ApiError::InternalServerError
        })?;
        let response = json!({
            "data": sensors,
        });
        return Ok(Json(response).into_response());
    } else if let Datastore::SQLite(sqlite) = &context.datastore {
        let sensors = sqlite.get_sensors().await.map_err(|err| {
            error!("Failed to get sensors from datastore: {:?}", err);
            ApiError::InternalServerError
        })?;
        let response = json!({
            "data": sensors,
        });
        return Ok(Json(response).into_response());
    } else {
        return Ok((StatusCode::NOT_IMPLEMENTED, "").into_response());
    }
}
