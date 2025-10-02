// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::error::AppError;
use crate::eventrepo;
use crate::eventrepo::EventRepo;
use crate::server::main::SessionExtractor;
use crate::server::ServerContext;
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde::Deserialize;
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StatsAggQuery {
    field: String,
    sensor_name: Option<String>,
    min_timestamp: Option<String>,
    max_timestamp: Option<String>,
}

impl StatsAggQuery {
    fn start_datetime(&self) -> anyhow::Result<DateTime> {
        if let Some(ref min_ts) = self.min_timestamp {
            return crate::datetime::parse(min_ts, None)
                .map_err(|e| anyhow::anyhow!("Failed to parse min_timestamp: {}", e));
        }

        // Default to 24 hours ago if no min_timestamp provided
        let delta = chrono::Duration::hours(24);
        let now = DateTime::now();
        Ok((now.datetime - delta).into())
    }

    fn end_datetime(&self) -> anyhow::Result<DateTime> {
        if let Some(ref max_ts) = self.max_timestamp {
            return crate::datetime::parse(max_ts, None)
                .map_err(|e| anyhow::anyhow!("Failed to parse max_timestamp: {}", e));
        }

        // Default to current time if no max_timestamp provided
        Ok(DateTime::now())
    }

    fn validate_timestamps(&self) -> anyhow::Result<()> {
        // Only validate if both absolute timestamps are provided
        if let (Some(min_ts), Some(max_ts)) = (&self.min_timestamp, &self.max_timestamp) {
            let start = crate::datetime::parse(min_ts, None)?;
            let end = crate::datetime::parse(max_ts, None)?;

            if start.datetime >= end.datetime {
                return Err(anyhow::anyhow!(
                    "Invalid time range: min_timestamp must be before max_timestamp"
                ));
            }
        }
        Ok(())
    }
}

pub(crate) async fn agg(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, AppError> {
    form.validate_timestamps()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let start_time = form.start_datetime().unwrap();
    let end_time = form.end_datetime().unwrap();
    let params = eventrepo::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        start_time,
        end_time,
    };

    match context.datastore.stats_agg(&params).await {
        Ok(response) => Ok(Json(response)),
        Err(err) => {
            error!(
                "Stats agg differential query failed: params={:?}, error={:?}",
                &params, err
            );
            Err(AppError::InternalServerError)
        }
    }
}

pub(crate) async fn agg_differential(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, AppError> {
    form.validate_timestamps()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let start_time = form.start_datetime().unwrap();
    let end_time = form.end_datetime().unwrap();
    let params = eventrepo::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        start_time,
        end_time,
    };

    match context.datastore.stats_agg_diff(&params).await {
        Ok(response) => Ok(Json(response)),
        Err(err) => {
            error!(
                "Stats agg differential query failed: params={:?}, error={:?}",
                &params, err
            );
            Err(AppError::InternalServerError)
        }
    }
}

// Doesn't really belong in this module.
pub(crate) async fn get_sensor_names(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    let sensors = if let EventRepo::Elastic(elastic) = &context.datastore {
        elastic.get_sensors().await.map_err(|err| {
            error!("Failed to get sensors: {:?}", err);
            AppError::InternalServerError
        })?
    } else if let EventRepo::SQLite(sqlite) = &context.datastore {
        sqlite.get_sensors().await.map_err(|err| {
            error!("Failed to get sensors: {:?}", err);
            AppError::InternalServerError
        })?
    } else {
        return Ok((StatusCode::NOT_IMPLEMENTED, "").into_response());
    };

    let response = json!({
        "data": sensors,
    });

    Ok(Json(response).into_response())
}

pub(crate) async fn earliest_timestamp(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    let ts = context.datastore.earliest_timestamp().await?;
    Ok(Json(ts))
}

pub(crate) async fn agg_by_sensor(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, AppError> {
    form.validate_timestamps()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let start_time = form.start_datetime().unwrap();
    let end_time = form.end_datetime().unwrap();
    let min_timestamp = start_time.to_rfc3339_utc();
    let max_timestamp = end_time.to_rfc3339_utc();
    let params = eventrepo::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: None, // We don't filter by sensor, we group by all sensors
        start_time,
        end_time,
    };

    match context.datastore.stats_agg_by_sensor(&params).await {
        Ok(response) => {
            let response_with_metadata = json!({
                "data": response.get("data"),
                "min_timestamp": min_timestamp,
                "max_timestamp": max_timestamp,
            });
            Ok(Json(response_with_metadata))
        }
        Err(err) => {
            error!(
                "Stats agg by sensor query failed: params={:?}, error={:?}",
                &params, err
            );
            Err(AppError::InternalServerError)
        }
    }
}

pub(crate) async fn agg_differential_by_sensor(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, AppError> {
    form.validate_timestamps()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let start_time = form.start_datetime().unwrap();
    let end_time = form.end_datetime().unwrap();
    let min_timestamp = start_time.to_rfc3339_utc();
    let max_timestamp = end_time.to_rfc3339_utc();
    let params = eventrepo::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: None, // We don't filter by sensor, we group by all sensors
        start_time,
        end_time,
    };

    match context.datastore.stats_agg_diff_by_sensor(&params).await {
        Ok(response) => {
            let response_with_metadata = json!({
                "data": response.get("data"),
                "min_timestamp": min_timestamp,
                "max_timestamp": max_timestamp,
            });
            Ok(Json(response_with_metadata))
        }
        Err(err) => {
            error!(
                "Stats agg differential by sensor query failed: params={:?}, error={:?}",
                &params, err
            );
            Err(AppError::InternalServerError)
        }
    }
}
