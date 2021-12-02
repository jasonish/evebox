// Copyright (C) 2021 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::datastore;
use crate::datastore::Datastore;
use crate::prelude::*;
use crate::server::api::ApiError;
use crate::server::main::AxumSessionExtractor;
use crate::server::ServerContext;
use axum::extract::{Extension, Form};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use time::Duration;

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

pub(crate) async fn get_sensor_names(
    _session: AxumSessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
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

pub(crate) async fn stats_agg(
    _session: AxumSessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> impl IntoResponse {
    let duration = form.duration();
    let start_time = form.start_time().unwrap();
    let params = datastore::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        duration,
        interval: bucket_interval(duration),
        start_time,
    };

    match &context.datastore {
        Datastore::Elastic(ds) => {
            let response = ds.stats_agg(params).await.unwrap();
            Json(response).into_response()
        }
        Datastore::SQLite(ds) => {
            let response = ds.stats_agg(params).await.unwrap();
            Json(response).into_response()
        }
        Datastore::None => (StatusCode::NOT_IMPLEMENTED, "not implemented").into_response(),
    }
}

pub(crate) async fn stats_derivative_agg(
    _session: AxumSessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(form): Form<StatsAggQuery>,
) -> impl IntoResponse {
    let duration = form.duration();
    let start_time = form.start_time().unwrap();
    let params = datastore::StatsAggQueryParams {
        field: form.field.to_string(),
        sensor_name: form.sensor_name.clone(),
        duration,
        interval: bucket_interval(duration),
        start_time,
    };
    return match &context.datastore {
        Datastore::Elastic(elastic) => {
            let response = elastic.stats_agg_deriv(params).await.unwrap();
            Json(response).into_response()
        }
        Datastore::SQLite(sqlite) => {
            let response = sqlite.stats_agg_deriv(params).await.unwrap();
            Json(response).into_response()
        }
        Datastore::None => (StatusCode::NOT_IMPLEMENTED, "not implemented").into_response(),
    };
}
