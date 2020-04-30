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
use warp::{reject, Filter};

use crate::server::api::ApiError;
use crate::server::session::Session;
use crate::server::ServerContext;

use super::api;

type DateTime = chrono::DateTime<chrono::Utc>;

#[derive(Deserialize, Debug, Default)]
pub struct GenericQuery {
    pub tags: Option<String>,
    pub time_range: Option<String>,
    pub query_string: Option<String>,
    pub min_ts: Option<String>,
    pub max_ts: Option<String>,
    pub order: Option<String>,
    pub event_type: Option<String>,
    pub sort_by: Option<String>,
    pub size: Option<u64>,
    pub interval: Option<String>,
    pub address_filter: Option<String>,
    pub dns_type: Option<String>,
    pub agg: Option<String>,
    pub sensor_name: Option<String>,

    #[serde(flatten)]
    pub other: std::collections::HashMap<String, String>,
}

impl GenericQuery {
    pub fn from_str(input: &str) -> Result<GenericQuery, ApiError> {
        let mut query: GenericQuery =
            serde_urlencoded::from_str(input).map_err(|_| ApiError::QueryStringParseError)?;
        query.fixup();
        Ok(query)
    }

    pub fn from_string(input: String) -> Result<GenericQuery, ApiError> {
        let mut query: GenericQuery =
            serde_urlencoded::from_str(&input).map_err(|_| ApiError::QueryStringParseError)?;
        query.fixup();
        Ok(query)
    }

    fn fixup(&mut self) {
        if self.time_range.is_none() {
            self.time_range = self.other.get("timeRange").map(String::from);
            self.other.remove("timeRange");
        }
        if self.event_type.is_none() {
            self.event_type = self.other.get("eventType").map(String::from);
            self.other.remove("eventType");
        }
        if self.address_filter.is_none() {
            self.address_filter = self.other.get("addressFilter").map(String::from);
            self.other.remove("addressFilter");
        }
        if self.dns_type.is_none() {
            self.dns_type = self.other.get("dnsType").map(String::from);
            self.other.remove("dnsType");
        }
        if self.query_string.is_none() {
            self.query_string = self.other.get("queryString").map(String::from);
            self.other.remove("queryString");
        }
        if self.sensor_name.is_none() {
            self.sensor_name = self.other.get("sensorFilter").map(String::from);
            self.other.remove("sensorFilter");
        }
    }

    pub fn mints_from_time_range(&self, now: &DateTime) -> Result<Option<DateTime>, ApiError> {
        if let Some(time_range) = &self.time_range {
            if time_range == "0s" {
                return Ok(None);
            }
            let duration = humantime::Duration::from_str(time_range)
                .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
            let duration = chrono::Duration::from_std(*duration.as_ref())
                .map_err(|_| ApiError::TimeRangeParseError(time_range.to_string()))?;
            let mints = now
                .checked_sub_signed(duration)
                .ok_or_else(|| ApiError::TimeRangeParseError(time_range.to_string()))?;
            Ok(Some(mints))
        } else {
            Ok(None)
        }
    }
}

/// A filter to get the raw query string as a String which does not fail
/// on an empty query string, instead returning an empty string.
fn query_string() -> impl Filter<Extract = (String,), Error = Infallible> + Clone {
    warp::any().and(
        warp::query::raw()
            .or(warp::any().map(|| "".to_string()))
            .unify(),
    )
}

fn generic_query() -> impl Filter<Extract = (GenericQuery,), Error = warp::Rejection> + Clone {
    query_string().and_then(|s: String| async move {
        if let Ok(qp) = GenericQuery::from_str(&s) {
            api::helpers::log_unknown_parameters("generic-query", &qp.other);
            Ok(qp)
        } else {
            Err(reject::custom(ApiError::QueryStringParseError))
        }
    })
}

#[allow(clippy::redundant_clone)]
pub fn api_routes(
    server_context: Arc<ServerContext>,
    session: warp::filters::BoxedFilter<(Arc<Session>,)>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let context_filter = warp::any().map(move || server_context.clone()).boxed();

    let api = warp::path("api")
        .and(warp::path("1"))
        .and(context_filter.clone());

    // Unauthenticated.
    let login_options = api
        .clone()
        .and(warp::path("login"))
        .and(warp::path::end())
        .and(warp::options())
        .and_then(api::login::options)
        .boxed();

    // Unauthenticated.
    let login_post = api
        .clone()
        .and(warp::path("login"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::form())
        .and_then(api::login::post);

    let logout = api
        .clone()
        .and(warp::path("logout"))
        .and(warp::path::end())
        .and(session.clone())
        .and_then(api::login::logout);

    // /api/1/version
    let get_version = api
        .clone()
        .and(warp::path("version"))
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .map(|_context, _session| {
            let version = serde_json::json!({
                "version": crate::version::version(),
                "revision": crate::version::build_rev(),
            });
            warp::reply::json(&version)
        })
        .boxed();

    let get_user = api
        .clone()
        .and(warp::path("user"))
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .map(|_context, session: Arc<Session>| {
            dbg!(&session);
            let user = serde_json::json!({
                "username": session.username(),
            });
            warp::reply::json(&user)
        })
        .boxed();

    // /api/1/config
    let get_config = api
        .clone()
        .and(warp::path("config"))
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .and_then(api::config)
        .boxed();

    // /api/1/alerts
    let get_alert_query = api
        .clone()
        .and(warp::path("alerts"))
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .and(generic_query())
        .and_then(api::alert_query)
        .boxed();

    // /api/1/event-query
    let get_event_query = api
        .clone()
        .and(warp::path("event-query"))
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .and(generic_query())
        .and_then(api::event_query)
        .boxed();

    // /api/1/alert-group/archive
    let post_alert_group_archive = api
        .clone()
        .and(warp::path("alert-group"))
        .and(warp::path("archive"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and(warp::body::json())
        .and_then(api::alert_group_archive)
        .boxed();

    // /api/1/alert-group/star
    let post_alert_group_star = api
        .clone()
        .and(warp::path("alert-group"))
        .and(warp::path("star"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and(warp::body::json())
        .and_then(api::alert_group_star)
        .boxed();

    // /api/1/alert-group/unstar
    let post_alert_group_unstar = api
        .clone()
        .and(warp::path("alert-group"))
        .and(warp::path("unstar"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and(warp::body::json())
        .and_then(api::alert_group_unstar)
        .boxed();

    // /api/1/event/:id
    let get_event_by_id = api
        .clone()
        .and(warp::path("event"))
        .and(warp::path::param())
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .and_then(api::get_event_by_id)
        .boxed();

    // /api/1/event/:id/archive
    let post_archive_event_by_id = api
        .clone()
        .and(warp::path("event"))
        .and(warp::path::param())
        .and(warp::path("archive"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and_then(api::archive_event_by_id)
        .boxed();

    // /api/1/event/:id/escalate
    let post_escalate_event_by_id = api
        .clone()
        .and(warp::path("event"))
        .and(warp::path::param())
        .and(warp::path("escalate"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and_then(api::escalate_event_by_id)
        .boxed();

    // /api/1/event/:id/de-escalate
    let post_deescalate_event_by_id = api
        .clone()
        .and(warp::path("event"))
        .and(warp::path::param())
        .and(warp::path("de-escalate"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and_then(api::deescalate_event_by_id)
        .boxed();

    // /api/1/event/:id/comment
    let post_comment_event_by_id = api
        .clone()
        .and(warp::path("event"))
        .and(warp::path::param())
        .and(warp::path("comment"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and(warp::body::json())
        .and_then(api::comment_by_event_id)
        .boxed();

    let post_query = api
        .clone()
        .and(warp::path("query"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and(warp::body::json())
        .and_then(api::query_elastic)
        .boxed();

    let post_alert_group_comment = api
        .clone()
        .and(warp::path("alert-group"))
        .and(warp::path("comment"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and(warp::body::json())
        .and_then(api::alert_group_comment)
        .boxed();

    let get_report_histogram = api
        .clone()
        .and(warp::path("report"))
        .and(warp::path("histogram"))
        .and(generic_query())
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .and_then(api::histogram)
        .boxed();

    let get_report_agg = api
        .clone()
        .and(warp::path("report"))
        .and(warp::path("agg"))
        .and(generic_query())
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .and_then(api::agg)
        .boxed();

    let eve2pcap = api
        .clone()
        .and(warp::path("eve2pcap"))
        .and(warp::path::end())
        .and(warp::post())
        .and(session.clone())
        .and(warp::body::form())
        .and_then(api::eve2pcap::handler)
        .boxed();

    let get_flow_histogram = api
        .clone()
        .and(warp::path("flow"))
        .and(warp::path("histogram"))
        .and(warp::query())
        .and(warp::path::end())
        .and(warp::get())
        .and(session.clone())
        .and_then(api::flow_histogram::handler)
        .boxed();

    let content_length_limit = 1024 * 1024 * 32;
    let submit = api
        .clone()
        .and(warp::path("submit"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::content_length_limit(content_length_limit))
        .and(warp::body::bytes())
        .and_then(api::submit::handler)
        .boxed();

    get_version
        .or(get_config)
        .or(get_alert_query)
        .or(get_event_query)
        .or(get_event_by_id)
        .or(post_archive_event_by_id)
        .or(post_escalate_event_by_id)
        .or(post_deescalate_event_by_id)
        .or(post_alert_group_archive)
        .or(post_alert_group_star)
        .or(post_alert_group_unstar)
        .or(post_query)
        .or(post_alert_group_comment)
        .or(post_comment_event_by_id)
        .or(get_report_histogram)
        .or(get_report_agg)
        .or(eve2pcap)
        .or(get_flow_histogram)
        .or(submit)
        .or(get_user)
        .or(login_options)
        .or(login_post)
        .or(logout)
        .boxed()
}
