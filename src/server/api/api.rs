// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;
use warp::Reply;

use crate::datastore::{self, EventQueryParams};
use crate::elastic;
use crate::logger::log;
use crate::server::filters::GenericQuery;
use crate::server::response::Response;
use crate::server::session::Session;
use crate::server::ServerContext;
use crate::{
    datastore::HistogramInterval,
    types::{DateTime, JsonValue},
};

#[derive(Deserialize, Debug, Clone)]
pub struct AlertGroupSpec {
    pub signature_id: u64,
    pub src_ip: String,
    pub dest_ip: String,
    pub min_timestamp: String,
    pub max_timestamp: String,
}

pub async fn config(
    context: Arc<ServerContext>,
    session: Arc<Session>,
) -> Result<impl warp::Reply, Infallible> {
    let config = json!({
       "ElasticSearchIndex": context.config.elastic_index,
       "event-services": context.event_services,
       "extra": {
            "elasticSearchKeywordSuffix": ".keyword",
       },
       "features": &context.features,
    });
    Ok(warp::reply::with_header(
        Response::Json(config).into_response(),
        "x-evebox-session-id",
        &session.session_id,
    ))
}

#[derive(Deserialize, Debug)]
pub struct ReportDhcpRequest {
    pub time_range: Option<String>,
    pub query_string: Option<String>,
}

pub async fn report_dhcp(
    context: Arc<ServerContext>,
    _session: Arc<Session>,
    what: String,
    request: ReportDhcpRequest,
) -> Result<impl warp::Reply, Infallible> {
    let mut params = EventQueryParams::default();
    if let Some(time_range) = request.time_range {
        let now = chrono::Utc::now();
        match parse_then_from_duration(&now, &time_range) {
            Some(then) => {
                params.min_timestamp = Some(then);
            }
            None => {
                log::warn!("Failed to parse time range: {}", time_range);
            }
        }
    }

    if let Some(query_string) = request.query_string {
        if !query_string.is_empty() {
            params.query_string = Some(query_string);
        }
    }

    match context.datastore.report_dhcp(&what, &params).await {
        Ok(response) => Ok(Response::Json(response)),
        Err(err) => Ok(Response::InternalError(err.to_string())),
    }
}

pub async fn alert_group_star(
    context: Arc<ServerContext>,
    session: Arc<Session>,
    request: AlertGroupSpec,
) -> Result<impl warp::Reply, Infallible> {
    log::info!("Escalated alert group: {:?}", request);
    context
        .datastore
        .escalate_by_alert_group(request, session)
        .await
        .unwrap();
    Ok(Response::Ok)
}

pub async fn alert_group_unstar(
    context: Arc<ServerContext>,
    _session: Arc<Session>,
    request: AlertGroupSpec,
) -> Result<impl warp::Reply, Infallible> {
    log::info!("De-escalating alert group: {:?}", request);
    context
        .datastore
        .deescalate_by_alert_group(request)
        .await
        .unwrap();
    Ok(Response::Ok)
}

pub async fn alert_group_archive(
    context: Arc<ServerContext>,
    _session: Arc<Session>,
    request: AlertGroupSpec,
) -> Result<impl warp::Reply, Infallible> {
    match context.datastore.archive_by_alert_group(request).await {
        Ok(_) => Ok(Response::Ok),
        Err(err) => {
            log::error!("Failed to archive by alert group: {}", err);
            Ok(Response::InternalError(err.to_string()))
        }
    }
}

pub async fn agg(
    context: Arc<ServerContext>,
    query: GenericQuery,
    _session: Arc<Session>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let min_timestamp = query.mints_from_time_range(&chrono::Utc::now())?;
    let agg = if let Some(agg) = query.agg {
        agg
    } else {
        return Err(ApiError::BadRequest("agg is a required parameter".to_string()).into());
    };
    let params = datastore::AggParameters {
        min_timestamp: min_timestamp,
        event_type: query.event_type,
        dns_type: query.dns_type,
        query_string: query.query_string,
        address_filter: query.address_filter,
        size: query.size.unwrap_or(10),
        agg: agg,
    };
    let agg = context.datastore.agg(params).await?;
    Ok(Response::Json(agg))
}

pub async fn histogram(
    context: Arc<ServerContext>,
    query: GenericQuery,
    _session: Arc<Session>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut params = datastore::HistogramParameters::default();
    let now = chrono::Utc::now();
    params.min_timestamp = query.mints_from_time_range(&now)?;
    if params.min_timestamp.is_some() {
        params.max_timestamp = Some(now);
    }
    if let Some(interval) = query.interval {
        let interval = HistogramInterval::from_str(&interval)
            .map_err(|_| ApiError::BadRequest(format!("failed to parse interval: {}", interval)))?;
        params.interval = Some(interval);
    }
    params.event_type = query.event_type;
    params.dns_type = query.dns_type;
    params.address_filter = query.address_filter;
    params.query_string = query.query_string;
    params.sensor_name = query.sensor_name;

    let response = context.datastore.histogram(params).await?;
    Ok(Response::Json(response))
}

pub async fn alert_query(
    context: Arc<ServerContext>,
    session: Arc<Session>,
    query: GenericQuery,
) -> Result<impl warp::Reply, Infallible> {
    let mut options = elastic::AlertQueryOptions::default();
    options.query_string = query.query_string;

    if let Some(tags) = query.tags {
        if !tags.is_empty() {
            let tags: Vec<String> = tags.split(',').map(|s| s.to_string()).collect();
            options.tags = tags;
        }
    }

    if let Some(time_range) = query.time_range {
        if !time_range.is_empty() {
            let now = chrono::Utc::now();
            match parse_then_from_duration(&now, &time_range) {
                None => {
                    log::error!("Failed to parse time_range: {}", time_range);
                }
                Some(then) => {
                    options.timestamp_gte = Some(then);
                }
            }
        }
    }

    if let Some(_ts) = query.min_ts {
        log::error!("alert_query: min_ts query argument not implemented");
    }

    if let Some(_ts) = query.max_ts {
        log::error!("alert_query: max_ts query argument not implemented");
    }

    match context.datastore.alert_query(options).await {
        Ok(v) => Ok(Response::Json(v).with_session(session)),
        Err(err) => {
            log::error!("alert query failed: {}", err);
            Ok(Response::InternalError(err.to_string()).with_session(session))
        }
    }
}

pub async fn get_event_by_id(
    context: Arc<ServerContext>,
    event_id: String,
    _session: Arc<Session>,
) -> Result<impl warp::Reply, Infallible> {
    match context.datastore.get_event_by_id(event_id.clone()).await {
        Err(err) => Ok(Response::InternalError(err.to_string())),
        Ok(Some(event)) => Ok(Response::Json(event)),
        Ok(None) => Ok(Response::NotFound),
    }
}

pub async fn archive_event_by_id(
    context: Arc<ServerContext>,
    event_id: String,
    _session: Arc<Session>,
) -> Result<impl warp::Reply, Infallible> {
    match context.datastore.archive_event_by_id(event_id).await {
        Ok(()) => Ok(Response::Ok),
        Err(err) => Ok(Response::InternalError(err.to_string())),
    }
}

pub async fn escalate_event_by_id(
    context: Arc<ServerContext>,
    event_id: String,
    _session: Arc<Session>,
) -> Result<impl warp::Reply, Infallible> {
    match context.datastore.escalate_event_by_id(event_id).await {
        Ok(()) => Ok(Response::Ok),
        Err(err) => Ok(Response::InternalError(err.to_string())),
    }
}

pub async fn deescalate_event_by_id(
    context: Arc<ServerContext>,
    event_id: String,
    _session: Arc<Session>,
) -> Result<impl warp::Reply, Infallible> {
    match context.datastore.deescalate_event_by_id(event_id).await {
        Ok(()) => Ok(Response::Ok),
        Err(err) => Ok(Response::InternalError(err.to_string())),
    }
}

pub async fn comment_by_event_id(
    context: Arc<ServerContext>,
    event_id: String,
    _session: Arc<Session>,
    body: EventCommentRequestBody,
) -> Result<impl warp::Reply, Infallible> {
    match context
        .datastore
        .comment_event_by_id(event_id, body.comment)
        .await
    {
        Ok(()) => Ok(Response::Ok),
        Err(err) => Ok(Response::InternalError(err.to_string())),
    }
}

/// REST API handler to perform a raw query against the Elastic Search server.
pub async fn query_elastic(
    context: Arc<ServerContext>,
    _session: Arc<Session>,
    body: serde_json::Value,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(elastic) = &context.elastic {
        let response = match elastic.search(&body).await {
            Err(err) => {
                return Ok(Response::InternalError(err.to_string()));
            }
            Ok(response) => response,
        };
        let response: JsonValue = match response.json().await {
            Err(err) => {
                return Ok(Response::InternalError(err.to_string()));
            }
            Ok(response) => response,
        };
        return Ok(Response::Json(response));
    }
    Ok(Response::Ok)
}

pub async fn event_query(
    context: Arc<ServerContext>,
    _session: Arc<Session>,
    query: GenericQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut params = datastore::EventQueryParams {
        size: query.size,
        sort_by: query.sort_by,
        event_type: query.event_type,
        ..Default::default()
    };

    if let Some(query_string) = query.query_string {
        if !query_string.is_empty() {
            params.query_string = Some(query_string);
        }
    }

    if let Some(order) = query.order {
        params.order = Some(order);
    }

    if let Some(ts) = query.min_ts {
        match parse_timestamp(&ts) {
            Ok(ts) => params.min_timestamp = Some(ts),
            Err(_) => {
                return Ok(Response::TimestampParseError(ts));
            }
        }
    }

    if let Some(ts) = query.max_ts {
        match parse_timestamp(&ts) {
            Ok(ts) => params.max_timestamp = Some(ts),
            Err(err) => {
                log::error!(
                    "event_query: failed to parse max timestamp: \"{}\": error={}",
                    &ts,
                    err
                );
                return Ok(Response::TimestampParseError(ts));
            }
        }
    }

    if query.time_range.is_some() {
        params.min_timestamp = super::helpers::mints_from_time_range(query.time_range, None)?;
    }

    match context.datastore.event_query(params).await {
        Err(err) => {
            log::error!("error: {}", err);
            Ok(Response::StatusCode(
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
        Ok(v) => Ok(Response::Json(v)),
    }
}

#[derive(Deserialize)]
pub struct EventCommentRequestBody {
    pub comment: String,
}

#[derive(Deserialize)]
pub struct AlertGroupCommentRequest {
    pub alert_group: AlertGroupSpec,
    pub comment: String,
}

pub async fn alert_group_comment(
    context: Arc<ServerContext>,
    _session: Arc<Session>,
    request: AlertGroupCommentRequest,
) -> Result<impl warp::Reply, Infallible> {
    match context
        .datastore
        .comment_by_alert_group(request.alert_group, request.comment)
        .await
    {
        Ok(()) => Ok(Response::Ok),
        Err(err) => Ok(Response::InternalError(err.to_string())),
    }
}

fn parse_then_from_duration(now: &DateTime, duration: &str) -> Option<DateTime> {
    // First parse to a somewhat standard duration.
    let duration = match humantime::Duration::from_str(duration) {
        Ok(duration) => duration,
        Err(err) => {
            log::error!("Failed to parse duration: {}: {}", duration, err);
            return None;
        }
    };
    // Convert the standard duration to a chrono duration.
    let duration = match chrono::Duration::from_std(*duration.as_ref()) {
        Ok(x) => x,
        Err(err) => {
            log::error!(
                "Failed to convert duration from humantime to chrono: {}: {}",
                duration,
                err
            );
            return None;
        }
    };
    now.checked_sub_signed(duration)
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("failed to parse time range: {0}")]
    TimeRangeParseError(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("failed to decode query string")]
    QueryStringParseError,
}

impl warp::reject::Reject for ApiError {}

impl From<ApiError> for warp::Rejection {
    fn from(err: ApiError) -> Self {
        warp::reject::custom(err)
    }
}

#[cfg(test)]
mod test {
    use crate::server::filters::GenericQuery;

    #[test]
    fn test_query_params_mints_from_time_range() {
        let now = chrono::Utc::now();

        let query = GenericQuery {
            ..Default::default()
        };
        let r = query.mints_from_time_range(&now).unwrap();
        dbg!(r);

        let query = GenericQuery {
            time_range: Some("3600s".to_string()),
            ..Default::default()
        };
        let r = query.mints_from_time_range(&now).unwrap();
        dbg!(r);
    }
}

fn parse_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, Box<dyn std::error::Error + Sync + Send>> {
    // The webapp may send the timestamp with an inproperly encoded +, which will be received
    // as space. Help the parsing out by replacing spaces with "+".
    let timestamp = timestamp.replace(" ", "+");
    let ts = percent_encoding::percent_decode_str(&timestamp).decode_utf8_lossy();
    crate::eve::parse_eve_timestamp(&ts)
}
