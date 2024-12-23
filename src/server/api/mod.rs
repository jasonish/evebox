// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::error::AppError;
use crate::eventrepo::EventQueryParams;
use crate::eventrepo::EventRepo;
use crate::queryparser::{QueryElement, QueryValue};
use crate::server::api::genericquery::GenericQuery;
use crate::server::main::SessionExtractor;
use crate::server::ServerContext;
use crate::{elastic, queryparser};
use axum::extract::{Extension, Form, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

use self::genericquery::TimeRange;
use self::util::parse_duration;

pub(crate) mod admin;
pub(crate) mod agg;
pub(crate) mod eve2pcap;
pub(crate) mod genericquery;
pub(crate) mod login;
pub(crate) mod sqlite;
pub(crate) mod stats;
pub(crate) mod submit;
pub(crate) mod util;

pub(crate) fn router() -> axum::Router<Arc<ServerContext>> {
    axum::Router::new()
        .route("/api/1/login", post(login::post).get(login::options))
        .route("/api/1/logout", post(login::logout))
        .route("/api/1/config", get(config))
        .route("/api/1/version", get(get_version))
        .route("/api/1/user", get(get_user))
        .route("/api/1/alerts", get(alerts))
        .route("/api/1/events", get(events))
        .route("/api/1/event/:id", get(get_event_by_id))
        .route("/api/1/alert-group/star", post(alert_group_star))
        .route("/api/1/alert-group/unstar", post(alert_group_unstar))
        .route("/api/1/alert-group/archive", post(alert_group_archive))
        .route("/api/1/event/:id/archive", post(archive_event_by_id))
        .route("/api/1/event/:id/escalate", post(escalate_event_by_id))
        .route("/api/event/:id/comment", post(comment_by_event_id))
        .route("/api/1/event/:id/de-escalate", post(deescalate_event_by_id))
        .route("/api/1/report/histogram/time", get(histogram_time))
        .route("/api/1/dhcp/ack", get(dhcp_ack))
        .route("/api/1/dhcp/request", get(dhcp_request))
        .route("/api/1/eve2pcap", post(eve2pcap::handler))
        .route("/api/1/submit", post(submit::handler))
        .route("/api/1/sensors", get(stats::get_sensor_names))
        .route("/api/agg", get(agg::agg))
        .route("/api/1/sqlite/info", get(sqlite::info))
        .route("/api/1/sqlite/fts/check", get(sqlite::fts_check))
        .route("/api/1/sqlite/fts/enable", post(sqlite::fts_enable))
        .route("/api/1/sqlite/fts/disable", post(sqlite::fts_disable))
        .route("/api/ja4db/:fingerprint", get(ja4db))
        .route("/api/admin/update/ja4db", post(admin::update_ja4db))
        .route("/api/find-dns", get(find_dns))
        .nest("/api/1/stats", stats::router())
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct AlertGroupSpec {
    pub signature_id: u64,
    pub src_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub min_timestamp: String,
    pub max_timestamp: String,
}

pub(crate) async fn config(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> impl IntoResponse {
    let datastore = match context.datastore {
        EventRepo::Elastic(_) => "elasticsearch",
        EventRepo::SQLite(_) => "sqlite",
    };
    let config = json!({
        "ElasticSearchIndex": context.config.elastic_index,
        "event-services": context.event_services,
        "defaults": &context.defaults,
        "datastore": datastore,
    });
    Json(config)
}

pub(crate) async fn get_user(SessionExtractor(session): SessionExtractor) -> impl IntoResponse {
    let user = json!({
        "username": session.username,
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

#[derive(Deserialize, Debug, Default)]
pub(crate) struct DhcpAckQuery {
    pub time_range: Option<TimeRange>,
    pub sensor: Option<String>,
}

pub(crate) async fn dhcp_ack(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(query): Form<DhcpAckQuery>,
) -> Result<impl IntoResponse, AppError> {
    let earliest = query
        .time_range
        .map(|x| x.parse_time_range_as_min_timestamp())
        .transpose()?;

    let response = match &context.datastore {
        EventRepo::Elastic(ds) => ds.dhcp_ack(earliest, query.sensor).await?,
        EventRepo::SQLite(ds) => ds.dhcp_ack(earliest, query.sensor).await?,
    };

    #[rustfmt::skip]
    let response = json!({
	      "events": response,
    });

    Ok(Json(response))
}

pub(crate) async fn dhcp_request(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Form(query): Form<DhcpAckQuery>,
) -> Result<impl IntoResponse, AppError> {
    let earliest = query
        .time_range
        .map(|x| x.parse_time_range_as_min_timestamp())
        .transpose()?;

    let response = match &context.datastore {
        EventRepo::Elastic(ds) => ds.dhcp_request(earliest, query.sensor).await?,
        EventRepo::SQLite(ds) => ds.dhcp_request(earliest, query.sensor).await?,
    };

    #[rustfmt::skip]
    let response = json!({
	"events": response,
    });

    Ok(Json(response))
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
    SessionExtractor(session): SessionExtractor,
    Json(request): Json<AlertGroupSpec>,
) -> impl IntoResponse {
    info!("De-escalating alert group: {:?}", request);
    context
        .datastore
        .deescalate_by_alert_group(session, request)
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

pub(crate) async fn histogram_time(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(query): Form<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let interval = query
        .interval
        .as_ref()
        .map(|v| parse_duration(v).map(|v| v.as_secs()))
        .transpose()
        .map_err(|err| AppError::BadRequest(format!("interval: {err}")))?;

    let default_tz_offset = query.tz_offset.as_deref();

    let mut query_string = query
        .query_string
        .clone()
        .map(|q| queryparser::parse(&q, default_tz_offset))
        .transpose()?
        .unwrap_or_default();

    if let Some(event_type) = &query.event_type {
        query_string.push(QueryElement {
            negated: false,
            value: QueryValue::KeyValue("event_type".to_string(), event_type.to_string()),
        })
    }

    if let Some(time_range) = &query.time_range {
        if !time_range.is_empty() {
            let min_timestamp = parse_duration(time_range)
                .map(|d| chrono::Utc::now().sub(d))
                .map_err(|err| AppError::BadRequest(format!("time_range: {err}")))?;
            query_string.push(QueryElement {
                negated: false,
                value: QueryValue::From(min_timestamp.into()),
            });
        }
    }

    let results = match &context.datastore {
        EventRepo::Elastic(ds) => ds.histogram_time(interval, &query_string).await,
        EventRepo::SQLite(ds) => ds.histogram_time(interval, &query_string).await,
    }
    .map_err(|err| {
        error!("Histogram/time error: params={:?}, error={:?}", &query, err);
        AppError::InternalServerError
    })?;

    Ok(Json(json!({ "data": results })))
}

pub(crate) async fn alerts(
    Extension(context): Extension<Arc<ServerContext>>,
    // Session required to get here.
    _session: SessionExtractor,
    Form(query): Form<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let mut options = elastic::AlertQueryOptions {
        query_string: query.query_string,
        sensor: query.sensor,
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

    Ok(Json(context.datastore.alerts(options).await?).into_response())
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
        .comment_event_by_id(&event_id, body.comment.to_string(), session)
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

/// Find a DNS record.
async fn find_dns(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let src_ip = form
        .get("src_ip")
        .ok_or_else(|| AppError::BadRequest("src_ip required".into()))?
        .to_string();

    let dest_ip = form
        .get("dest_ip")
        .ok_or_else(|| AppError::BadRequest("dest_ip required".into()))?
        .to_string();

    let host = form.get("host").cloned();

    let before = form
        .get("before")
        .map(|ts| {
            crate::datetime::parse(ts, None).map_err(|_| anyhow::anyhow!("failed to parse before"))
        })
        .transpose()?;

    match &context.datastore {
        EventRepo::Elastic(e) => {
            let response = e.dns_reverse_lookup(before, host, src_ip, dest_ip).await?;
            Ok(Json(response))
        }
        EventRepo::SQLite(s) => {
            let response = s
                .dns_reverse_lookup(before.clone(), host, src_ip, dest_ip)
                .await?;
            Ok(Json(response))
        }
    }
}

pub(crate) async fn events(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(query): Form<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let mut params = EventQueryParams {
        size: query.size,
        sort_by: query.sort_by,
        event_type: query.event_type,
        order: query.order,
        ..Default::default()
    };

    if let Some(ts) = &query.min_timestamp {
        warn!("Deprecated field 'min_timestamp' in event query ({})", ts);
    }

    if let Some(ts) = &query.max_timestamp {
        warn!("Deprecated field 'max_timestamp' in event query ({})", ts);
    }

    let default_tz_offset = query.tz_offset.as_deref();

    let query_string = query
        .query_string
        .map(|qs| queryparser::parse(&qs, default_tz_offset))
        .transpose()?
        .unwrap_or_default();
    params.query_string = query_string;

    let results = context.datastore.events(params).await?;
    Ok(Json(results).into_response())
}

async fn ja4db(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(fingerprint): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let sql = "SELECT data FROM ja4db WHERE fingerprint = ?";
    let entry: Option<serde_json::Value> = sqlx::query_scalar(sql)
        .bind(fingerprint)
        .fetch_optional(&context.config_repo.pool)
        .await?;
    if let Some(entry) = entry {
        Ok(Json(entry).into_response())
    } else {
        let response = json!({
            "message": "fingerprint not found",
        });
        Ok((StatusCode::NOT_FOUND, Json(response)).into_response())
    }
}

#[derive(Deserialize)]
pub(crate) struct EventCommentRequestBody {
    pub comment: String,
}

fn parse_then_from_duration(
    now: &crate::datetime::DateTime,
    duration: &str,
) -> Option<crate::datetime::DateTime> {
    // First parse to a somewhat standard duration.
    let duration = match humantime::Duration::from_str(duration) {
        Ok(duration) => duration,
        Err(err) => {
            error!("Failed to parse duration: {}: {}", duration, err);
            return None;
        }
    };
    let d: std::time::Duration = duration.into();
    let d = chrono::Duration::from_std(d).unwrap();
    Some(now.sub(d))
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let err = self.to_string();
        warn!("API error: {:?}", self);
        match self {
            AppError::BadRequest(msg) => {
                let body = Json(serde_json::json!({
                    "error": msg,
                }));
                (StatusCode::BAD_REQUEST, body).into_response()
            }
            _ => {
                let body = Json(serde_json::json!({
                    "error": err,
                }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        }
    }
}
