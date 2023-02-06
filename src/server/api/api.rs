// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use axum::extract::{Extension, Form, Path};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;

use crate::prelude::*;
use serde::Deserialize;
use serde_json::json;

use crate::datastore::HistogramInterval;
use crate::datastore::{self, EventQueryParams};
use crate::searchquery::Element;
use crate::server::filters::GenericQuery;
use crate::server::main::SessionExtractor;
use crate::server::ServerContext;
use crate::{elastic, searchquery};

#[derive(Deserialize, Debug, Clone)]
pub struct AlertGroupSpec {
    pub signature_id: u64,
    pub src_ip: String,
    pub dest_ip: String,
    pub min_timestamp: String,
    pub max_timestamp: String,
}

pub(crate) async fn config(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> impl IntoResponse {
    let config = json!({
        "ElasticSearchIndex": context.config.elastic_index,
        "event-services": context.event_services,
        "extra": {
            "elasticSearchKeywordSuffix": ".keyword",
        },
        "features": &context.features,
        "defaults": &context.defaults,
    });
    Json(config)
}

pub(crate) async fn get_user(SessionExtractor(session): SessionExtractor) -> impl IntoResponse {
    let user = json!({
        "username": session.username(),
    });
    Json(user)
}

pub(crate) async fn get_version() -> impl IntoResponse {
    let version = serde_json::json!({
        "version": crate::version::version(),
        "revision": crate::version::build_rev(),
    });
    axum::Json(version)
}

#[derive(Deserialize, Debug)]
pub struct ReportDhcpRequest {
    pub time_range: Option<String>,
    pub query_string: Option<String>,
}

pub(crate) async fn report_dhcp(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(_session): SessionExtractor,
    Path(what): Path<String>,
    Form(request): Form<ReportDhcpRequest>,
) -> impl IntoResponse {
    let mut params = EventQueryParams::default();
    if let Some(time_range) = request.time_range {
        let now = time::OffsetDateTime::now_utc();
        match parse_then_from_duration(&now, &time_range) {
            Some(then) => {
                params.min_timestamp = Some(then);
            }
            None => {
                warn!("Failed to parse time range: {}", time_range);
            }
        }
    }

    if let Some(query_string) = request.query_string {
        if !query_string.is_empty() {
            params.query_string = Some(query_string);
        }
    }

    match context.datastore.report_dhcp(&what, &params).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub(crate) async fn alert_group_star(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Json(request): Json<AlertGroupSpec>,
) -> impl IntoResponse {
    info!("Escalated alert group: {:?}", request);
    context
        .datastore
        .escalate_by_alert_group(request, session)
        .await
        .unwrap();
    StatusCode::OK
}

pub(crate) async fn alert_group_unstar(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(_session): SessionExtractor,
    Json(request): Json<AlertGroupSpec>,
) -> impl IntoResponse {
    info!("De-escalating alert group: {:?}", request);
    context
        .datastore
        .deescalate_by_alert_group(request)
        .await
        .unwrap();
    StatusCode::OK
}

pub(crate) async fn alert_group_archive(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(_session): SessionExtractor,
    Json(request): Json<AlertGroupSpec>,
) -> impl IntoResponse {
    match context.datastore.archive_by_alert_group(request).await {
        Ok(_) => StatusCode::OK,
        Err(err) => {
            error!("Failed to archive by alert group: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub(crate) async fn agg(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(_session): SessionExtractor,
    Form(query): Form<GenericQuery>,
) -> impl IntoResponse {
    let min_timestamp = match query.mints_from_time_range(&time::OffsetDateTime::now_utc()) {
        Ok(ts) => ts,
        Err(err) => {
            error!("api/agg: {:?}", err);
            return (
                StatusCode::BAD_REQUEST,
                "failed to parse time range".to_string(),
            )
                .into_response();
        }
    };
    let agg = if let Some(agg) = query.agg {
        agg
    } else {
        return (StatusCode::BAD_REQUEST, "agg is a required parameter").into_response();
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
    match context.datastore.agg(params).await {
        Err(err) => {
            error!("Aggregation failed: {:?}", err);
            return (StatusCode::INTERNAL_SERVER_ERROR, "").into_response();
        }
        Ok(agg) => Json(agg).into_response(),
    }
}

pub(crate) async fn histogram(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(_session): SessionExtractor,
    Form(query): Form<GenericQuery>,
) -> impl IntoResponse {
    let mut params = datastore::HistogramParameters::default();
    let now = time::OffsetDateTime::now_utc();
    params.min_timestamp = query.mints_from_time_range(&now).unwrap();
    if params.min_timestamp.is_some() {
        params.max_timestamp = Some(now);
    }
    if let Some(interval) = query.interval {
        let interval = HistogramInterval::from_str(&interval)
            .map_err(|_| ApiError::BadRequest(format!("failed to parse interval: {interval}")))
            .unwrap();
        params.interval = Some(interval);
    }
    params.event_type = query.event_type;
    params.dns_type = query.dns_type;
    params.address_filter = query.address_filter;
    params.query_string = query.query_string;
    params.sensor_name = query.sensor_name;

    let response = context.datastore.histogram(params).await.unwrap();
    Json(response)
}

pub(crate) async fn alerts(
    Extension(context): Extension<Arc<ServerContext>>,
    // Session required to get here.
    _session: SessionExtractor,
    Form(query): Form<GenericQuery>,
) -> impl IntoResponse {
    let mut options = elastic::AlertQueryOptions {
        query_string: query.query_string,
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
            let now = time::OffsetDateTime::now_utc();
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

    match context.datastore.alerts(options).await {
        Ok(v) => axum::Json(v).into_response(),
        Err(err) => {
            error!("alert query failed: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
        }
    }
}

pub(crate) async fn get_event_by_id(
    Extension(context): Extension<Arc<ServerContext>>,
    Path(event_id): axum::extract::Path<String>,
    _session: SessionExtractor,
) -> impl IntoResponse {
    match context.datastore.get_event_by_id(event_id.clone()).await {
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        Ok(Some(event)) => Json(event).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

pub(crate) async fn archive_event_by_id(
    Extension(context): Extension<Arc<ServerContext>>,
    Path(event_id): axum::extract::Path<String>,
    _session: SessionExtractor,
) -> impl IntoResponse {
    match context.datastore.archive_event_by_id(&event_id).await {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            error!(
                "Failed to archive event by ID: id={}, err={:?}",
                event_id, err
            );
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub(crate) async fn escalate_event_by_id(
    Extension(context): Extension<Arc<ServerContext>>,
    Path(event_id): axum::extract::Path<String>,
    _session: SessionExtractor,
) -> impl IntoResponse {
    match context.datastore.escalate_event_by_id(&event_id).await {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            error!(
                "Failed to escalate event by ID: id={}, err={:?}",
                event_id, err
            );
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub(crate) async fn deescalate_event_by_id(
    Extension(context): Extension<Arc<ServerContext>>,
    Path(event_id): axum::extract::Path<String>,
    _session: SessionExtractor,
) -> impl IntoResponse {
    match context.datastore.deescalate_event_by_id(&event_id).await {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            error!(
                "Failed to de-escalate event by ID: id={}, err={:?}",
                event_id, err
            );
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub(crate) async fn comment_by_event_id(
    Extension(context): Extension<Arc<ServerContext>>,
    Path(event_id): axum::extract::Path<String>,
    SessionExtractor(session): SessionExtractor,
    Json(body): Json<EventCommentRequestBody>,
) -> impl IntoResponse {
    match context
        .datastore
        .comment_event_by_id(&event_id, body.comment.to_string(), session.username())
        .await
    {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            error!(
                "Failed to add comment by event ID: id={}, err={:?}",
                event_id, err
            );
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub(crate) async fn events(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(query): Form<GenericQuery>,
) -> impl IntoResponse {
    let params = match generic_query_to_event_query(&query) {
        Ok(params) => params,
        Err(err) => {
            return (StatusCode::BAD_REQUEST, format!("error: {err}")).into_response();
        }
    };

    match context.datastore.events(params).await {
        Err(err) => {
            error!("error: {:?}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
        }
        Ok(v) => Json(v).into_response(),
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

pub(crate) async fn alert_group_comment(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Json(request): Json<AlertGroupCommentRequest>,
) -> impl IntoResponse {
    match context
        .datastore
        .comment_by_alert_group(request.alert_group, request.comment, session.username())
        .await
    {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            info!("Failed to apply command to alert-group: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn parse_then_from_duration(
    now: &time::OffsetDateTime,
    duration: &str,
) -> Option<time::OffsetDateTime> {
    // First parse to a somewhat standard duration.
    let duration = match humantime::Duration::from_str(duration) {
        Ok(duration) => duration,
        Err(err) => {
            error!("Failed to parse duration: {}: {}", duration, err);
            return None;
        }
    };

    Some(now.sub(*duration.as_ref()))
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("failed to parse time range: {0}")]
    TimeRangeParseError(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("failed to decode query string")]
    QueryStringParseError,
    #[error("internal server error")]
    InternalServerError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::TimeRangeParseError(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::QueryStringParseError => (
                StatusCode::BAD_REQUEST,
                "query string parse error".to_string(),
            ),
            ApiError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_string(),
            ),
        };
        let body = Json(serde_json::json!({
            "error": message,
        }));
        (status, body).into_response()
    }
}

fn parse_timestamp2(timestamp: &str, offset: Option<&str>) -> Result<time::OffsetDateTime> {
    if let Some(timestamp) = timestamp_preprocess(timestamp, offset) {
        Ok(crate::eve::parse_eve_timestamp(&timestamp)?)
    } else {
        bail!("badly formatted date")
    }
}

/// Preprocesses a user supplied timestamp into something that can be parsed by the stricter
/// EVE timestamp format parser.
///
/// Requires at last a year.
fn timestamp_preprocess(s: &str, offset: Option<&str>) -> Option<String> {
    let default_offset = offset.unwrap_or("+0000");
    let re = regex::Regex::new(
        r"^(\d{4})-?(\d{2})?-?(\d{2})?[T ]?(\d{2})?:?(\d{2})?:?(\d{2})?(\.(\d+))?(([+\-]\d{4})|Z)?$",
    )
    .unwrap();
    if let Some(c) = re.captures(s) {
        let year = c.get(1).map_or("", |m| m.as_str());
        let month = c.get(2).map_or("01", |m| m.as_str());
        let day = c.get(3).map_or("01", |m| m.as_str());
        let hour = c.get(4).map_or("00", |m| m.as_str());
        let minute = c.get(5).map_or("00", |m| m.as_str());
        let second = c.get(6).map_or("00", |m| m.as_str());
        let subs = c.get(8).map_or("0", |m| m.as_str());
        let offset = c.get(9).map_or(default_offset, |m| m.as_str());
        return Some(format!(
            "{}-{}-{}T{}:{}:{}.{}{}",
            year,
            month,
            day,
            hour,
            minute,
            second,
            subs,
            if offset == "Z" { "+0000" } else { offset },
        ));
    }
    None
}

#[cfg(test)]
mod test {
    use crate::server::api::api::timestamp_preprocess;
    use crate::server::filters::GenericQuery;

    #[test]
    fn test_timestamp_preprocess() {
        assert_eq!(
            timestamp_preprocess("2023", None).unwrap(),
            "2023-01-01T00:00:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01", None).unwrap(),
            "2023-01-01T00:00:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01", None).unwrap(),
            "2023-01-01T01:00:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02", None).unwrap(),
            "2023-01-01T01:02:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01 01:02", None).unwrap(),
            "2023-01-01T01:02:00.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03", None).unwrap(),
            "2023-01-01T01:02:03.0+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123", None).unwrap(),
            "2023-01-01T01:02:03.123+0000"
        );
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123-0600", None).unwrap(),
            "2023-01-01T01:02:03.123-0600"
        );

        // Timezone offset without sub-seconds.
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03-0600", None).unwrap(),
            "2023-01-01T01:02:03.0-0600"
        );

        // '-' in the date and ':' in the time are optional.
        assert_eq!(
            timestamp_preprocess("20230101T010203.123-0600", None).unwrap(),
            "2023-01-01T01:02:03.123-0600"
        );

        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123", Some("+1200")).unwrap(),
            "2023-01-01T01:02:03.123+1200"
        );

        // 'Z' as timezone offset.
        assert_eq!(
            timestamp_preprocess("2023-01-01T01:02:03.123Z", None).unwrap(),
            "2023-01-01T01:02:03.123+0000"
        );
    }

    #[test]
    fn test_query_params_mints_from_time_range() {
        let now = time::OffsetDateTime::now_utc();

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

fn generic_query_to_event_query(query: &GenericQuery) -> anyhow::Result<EventQueryParams> {
    let mut params = EventQueryParams {
        size: query.size,
        sort_by: query.sort_by.clone(),
        event_type: query.event_type.clone(),
        order: query.order.clone(),
        ..Default::default()
    };

    let default_tz_offset: Option<&str> = query.tz_offset.as_ref().map(|s| s.as_ref());

    if let Some(query_string) = &query.query_string {
        let parts = parse_query_string(query_string, default_tz_offset)?;
        params.min_timestamp = parts.after;
        params.max_timestamp = parts.before;
        params.query_string_elements = parts.elements;
    }

    if let Some(min_timestamp) = &query.min_timestamp {
        if params.min_timestamp.is_some() {
            debug!("Ignoring min_timestamp, @after provided in query string");
        } else {
            match parse_timestamp2(min_timestamp, default_tz_offset) {
                Ok(ts) => params.min_timestamp = Some(ts),
                Err(err) => {
                    error!(
                        "event_query: failed to parse max timestamp: \"{}\": error={}",
                        &min_timestamp, err
                    );
                    bail!(
                        "failed to parse min_timestamp: {}, error={:?}",
                        &min_timestamp,
                        err
                    );
                }
            }
        }
    }

    if let Some(max_timestamp) = &query.max_timestamp {
        if params.max_timestamp.is_some() {
            debug!("Ignoring max_timestamp, @before provided in query string");
        } else {
            match parse_timestamp2(max_timestamp, default_tz_offset) {
                Ok(ts) => params.max_timestamp = Some(ts),
                Err(err) => {
                    error!(
                        "event_query: failed to parse max timestamp: \"{}\": error={}",
                        &max_timestamp, err
                    );
                    bail!(
                        "failed to parse max_timestamp: {}, error={:?}",
                        &max_timestamp,
                        err
                    );
                }
            }
        }
    }

    if params.min_timestamp.is_none() && query.time_range.is_some() {
        match super::helpers::mints_from_time_range(query.time_range.clone(), None) {
            Ok(ts) => {
                params.min_timestamp = ts;
            }
            Err(err) => {
                error!(
                    "Failed to parse time_range to timestamp: {:?}: {:?}",
                    query.time_range, err
                );
                bail!(
                    "failed to parse time_range: {:?}, error={:?}",
                    query.time_range,
                    err
                );
            }
        }
    }

    Ok(params)
}

#[derive(Default, Debug)]
pub struct QueryStringParts {
    pub before: Option<time::OffsetDateTime>,
    pub after: Option<time::OffsetDateTime>,
    pub elements: Vec<searchquery::Element>,
}

pub fn parse_query_string(query: &str, tz_offset: Option<&str>) -> Result<QueryStringParts> {
    let mut parts = QueryStringParts::default();
    match searchquery::parse(query) {
        Err(err) => {
            bail!("Failed to parse query string: {:?}", err);
        }
        Ok((_, elements)) => {
            for element in elements {
                match element {
                    Element::KeyVal(ref key, ref val) => match key.as_ref() {
                        "@before" => match parse_timestamp2(val, tz_offset) {
                            Ok(timestamp) => {
                                parts.before = Some(timestamp);
                            }
                            Err(err) => {
                                error!(
                                    "Failed to parse @after timestamp: {}, error={:?}",
                                    &val, err
                                );
                            }
                        },
                        "@after" => match parse_timestamp2(val, tz_offset) {
                            Ok(timestamp) => {
                                parts.after = Some(timestamp);
                            }
                            Err(err) => {
                                error!(
                                    "Failed to parse @after timestamp: {}, error={:?}",
                                    &val, err
                                );
                            }
                        },
                        _ => {
                            parts.elements.push(element);
                        }
                    },
                    _ => {
                        parts.elements.push(element);
                    }
                }
            }
        }
    }
    Ok(parts)
}
