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
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use std::sync::Arc;
use tracing::error;

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
    fn start_datetime(&self) -> anyhow::Result<DateTime> {
        let start_time = if let Some(time_range) = self.time_range {
            if time_range == 0 {
                let then = chrono::DateTime::UNIX_EPOCH;
                then.fixed_offset()
            } else {
                let delta = chrono::Duration::seconds(time_range);
                let now = DateTime::now();

                now.datetime - delta
            }
        } else {
            let delta = chrono::Duration::hours(24);
            let now = DateTime::now();

            now.datetime - delta
        };
        Ok(start_time.into())
    }
}

async fn agg(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, AppError> {
    let start_time = form.start_datetime().unwrap();
    let params = eventrepo::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        start_time,
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

async fn agg_differential(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> Result<impl IntoResponse, AppError> {
    let start_time = form.start_datetime().unwrap();
    let params = eventrepo::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        start_time,
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
