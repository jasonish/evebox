// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Events that share a flow with a given event.
//!
//! Suricata tags every event it logs for a flow (alerts, app-layer
//! records, fileinfo, anomalies and the final flow record) with the same
//! `flow_id`. The lookup is bound to a time window around the flow.
//!
//! It is not bound to the sensor: with Elastic Common Schema indices the
//! event's `host` and the field a `host` query maps to (`agent.name`) can
//! differ. Suricata flow IDs mix the flow hash with its start time into
//! 51 bits, so collisions with another sensor's flow inside the window
//! are negligible.
//!
//! A flow can raise the same alert thousands of times, so alerts are
//! returned grouped by signature with their true counts, while other
//! events are listed individually.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Response;

use super::prelude::*;
use crate::datetime::{self, DateTime};
use crate::error::AppError;
use crate::eventrepo::EventQueryParams;
use crate::prelude::*;
use crate::queryparser::{QueryElement, QueryValue};

/// Maximum number of non-alert events listed for one flow.
const MAX_EVENTS: u64 = 100;

/// Maximum number of distinct alert signatures returned for one flow.
const MAX_SIGNATURES: usize = 20;

/// Maximum number of event types counted for one flow.
const MAX_EVENT_TYPES: usize = 50;

/// Slack around known flow start and end times, for events logged
/// slightly outside them.
const SLACK: chrono::TimeDelta = chrono::TimeDelta::minutes(1);

/// How far to look from the event when the flow start or end is not
/// known.
const UNKNOWN_BOUND: chrono::TimeDelta = chrono::TimeDelta::hours(24);

/// Fields dropped from returned events; they are large and the flow
/// listing does not use them.
const STRIPPED_FIELDS: &[&str] = &["payload", "payload_printable", "packet"];

#[derive(Debug, PartialEq, Eq)]
struct FlowQuery {
    flow_id: String,
    from: DateTime,
    to: DateTime,
}

impl FlowQuery {
    /// Query elements selecting the flow's events.
    fn elements(&self) -> Vec<QueryElement> {
        vec![
            key_value("flow_id", &self.flow_id),
            QueryElement {
                negated: false,
                value: QueryValue::From(self.from.clone()),
            },
            QueryElement {
                negated: false,
                value: QueryValue::To(self.to.clone()),
            },
        ]
    }
}

fn key_value(key: &str, value: &str) -> QueryElement {
    QueryElement {
        negated: false,
        value: QueryValue::KeyValue(key.to_string(), value.to_string()),
    }
}

pub(crate) async fn flow_events(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(event_id): Path<String>,
) -> Result<Response, AppError> {
    let Some(event) = context.datastore.get_event_by_id(event_id).await? else {
        return Ok((StatusCode::NOT_FOUND, "not found").into_response());
    };

    let Some(query) = flow_query(&event["_source"]) else {
        return Ok(Json(json!({
            "flow_id": null,
            "counts": {},
            "alerts": [],
            "events": [],
            "flow": null,
            "truncated": false,
        }))
        .into_response());
    };

    let not_alert = QueryElement {
        negated: true,
        value: QueryValue::KeyValue("event_type".to_string(), "alert".to_string()),
    };
    let (counts, alerts, mut events) = tokio::try_join!(
        context
            .datastore
            .agg("event_type", MAX_EVENT_TYPES, "desc", query.elements()),
        alert_groups(&context, &query),
        find_events(&context, &query, vec![not_alert], "asc", MAX_EVENTS + 1),
    )?;

    let truncated = events.len() as u64 > MAX_EVENTS;
    events.truncate(MAX_EVENTS as usize);

    let counts: serde_json::Map<String, serde_json::Value> = counts
        .into_iter()
        .filter_map(|row| Some((agg_key(&row["key"])?, row["count"].clone())))
        .collect();

    // The flow record is logged when the flow ends, so it can fall past
    // the end of a truncated listing.
    let is_flow = |event: &serde_json::Value| event["_source"]["event_type"] == "flow";
    let flow = match events.iter().find(|event| is_flow(event)) {
        Some(flow) => Some(flow.clone()),
        None if truncated => find_events(
            &context,
            &query,
            vec![key_value("event_type", "flow")],
            "asc",
            1,
        )
        .await?
        .into_iter()
        .next(),
        None => None,
    };

    Ok(Json(json!({
        "flow_id": query.flow_id,
        "counts": counts,
        "alerts": alerts,
        "events": events,
        "flow": flow,
        "truncated": truncated,
    }))
    .into_response())
}

/// The flow's alerts grouped by signature: the count, the first
/// occurrence and the timestamp of the last.
async fn alert_groups(
    context: &ServerContext,
    query: &FlowQuery,
) -> Result<Vec<serde_json::Value>> {
    let mut elements = query.elements();
    elements.push(key_value("event_type", "alert"));
    let rows = context
        .datastore
        .agg("alert.signature_id", MAX_SIGNATURES, "desc", elements)
        .await?;

    let groups = rows.into_iter().filter_map(|row| {
        let signature_id = agg_key(&row["key"])?;
        let count = row["count"].as_u64()?;
        Some(async move {
            let filter = || vec![key_value("alert.signature_id", &signature_id)];
            let (first, last) = tokio::try_join!(
                find_events(context, query, filter(), "asc", 1),
                find_events(context, query, filter(), "desc", 1),
            )?;
            let Some(first) = first.into_iter().next() else {
                return Ok(None);
            };
            let last_timestamp = last
                .first()
                .map(|last| last["_source"]["timestamp"].clone())
                .unwrap_or_else(|| first["_source"]["timestamp"].clone());
            Ok::<_, anyhow::Error>(Some(json!({
                "count": count,
                "first": first,
                "last_timestamp": last_timestamp,
            })))
        })
    });

    Ok(futures::future::try_join_all(groups)
        .await?
        .into_iter()
        .flatten()
        .collect())
}

/// Aggregation keys are strings on SQLite and may be numbers on
/// Elasticsearch.
fn agg_key(key: &serde_json::Value) -> Option<String> {
    match key {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Events of the flow matching extra query elements, without their large
/// fields.
async fn find_events(
    context: &ServerContext,
    query: &FlowQuery,
    extra: Vec<QueryElement>,
    order: &str,
    size: u64,
) -> Result<Vec<serde_json::Value>> {
    let mut query_string = query.elements();
    query_string.extend(extra);
    let params = EventQueryParams {
        order: Some(order.to_string()),
        size: Some(size),
        query_string,
        ..Default::default()
    };

    let mut response = context.datastore.events(params).await?;
    let mut events = match response["events"].take() {
        serde_json::Value::Array(events) => events,
        _ => vec![],
    };
    for event in &mut events {
        if let Some(source) = event["_source"].as_object_mut() {
            for field in STRIPPED_FIELDS {
                source.remove(*field);
            }
        }
    }
    Ok(events)
}

/// Build the flow lookup for an event, or None if it has no flow ID or
/// timestamp.
fn flow_query(source: &serde_json::Value) -> Option<FlowQuery> {
    let flow_id = match &source["flow_id"] {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) if s.parse::<u64>().is_ok() => s.clone(),
        _ => return None,
    };
    let timestamp = parse_field(&source["timestamp"])?;

    // Flow events carry the flow's start and end; alerts and app-layer
    // events may carry its start.
    let flow = if source["flow"].is_object() {
        &source["flow"]
    } else {
        &source["netflow"]
    };
    let start = parse_field(&flow["start"]);
    let end = parse_field(&flow["end"]);

    let from = match start {
        Some(start) if start < timestamp => start.sub(SLACK),
        Some(_) => timestamp.sub(SLACK),
        None => timestamp.sub(UNKNOWN_BOUND),
    };
    let to = match end {
        Some(end) if end > timestamp => end.add(SLACK),
        Some(_) => timestamp.add(SLACK),
        None => timestamp.add(UNKNOWN_BOUND),
    };

    Some(FlowQuery { flow_id, from, to })
}

fn parse_field(value: &serde_json::Value) -> Option<DateTime> {
    value.as_str().and_then(|s| datetime::parse(s, None).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime {
        datetime::parse(s, None).unwrap()
    }

    #[test]
    fn test_no_flow_id() {
        let source = json!({"timestamp": "2026-10-10T12:00:00.000000+0000"});
        assert_eq!(flow_query(&source), None);
    }

    #[test]
    fn test_no_timestamp() {
        let source = json!({"flow_id": 1234});
        assert_eq!(flow_query(&source), None);
    }

    #[test]
    fn test_unknown_flow_times() {
        let source = json!({
            "flow_id": 1234,
            "host": "sensor1",
            "timestamp": "2026-10-10T12:00:00.000000+0000",
        });
        let query = flow_query(&source).unwrap();
        assert_eq!(query.flow_id, "1234");
        assert_eq!(query.from, ts("2026-10-09T12:00:00Z"));
        assert_eq!(query.to, ts("2026-10-11T12:00:00Z"));
    }

    #[test]
    fn test_alert_with_flow_start() {
        let source = json!({
            "event_type": "alert",
            "flow_id": 1234,
            "timestamp": "2026-10-10T12:00:00.000000+0000",
            "flow": {"start": "2026-10-10T11:30:00.000000+0000"},
        });
        let query = flow_query(&source).unwrap();
        assert_eq!(query.from, ts("2026-10-10T11:29:00Z"));
        assert_eq!(query.to, ts("2026-10-11T12:00:00Z"));
    }

    #[test]
    fn test_flow_event() {
        let source = json!({
            "event_type": "flow",
            "flow_id": "1234",
            "host": "",
            "timestamp": "2026-10-10T12:00:05.000000+0000",
            "flow": {
                "start": "2026-10-10T11:00:00.000000+0000",
                "end": "2026-10-10T12:00:00.000000+0000",
            },
        });
        let query = flow_query(&source).unwrap();
        assert_eq!(query.flow_id, "1234");
        assert_eq!(query.from, ts("2026-10-10T10:59:00Z"));
        assert_eq!(query.to, ts("2026-10-10T12:01:05Z"));
    }

    #[test]
    fn test_netflow_event() {
        let source = json!({
            "event_type": "netflow",
            "flow_id": 1234,
            "timestamp": "2026-10-10T12:00:00.000000+0000",
            "netflow": {
                "start": "2026-10-10T11:00:00.000000+0000",
                "end": "2026-10-10T12:30:00.000000+0000",
            },
        });
        let query = flow_query(&source).unwrap();
        assert_eq!(query.from, ts("2026-10-10T10:59:00Z"));
        assert_eq!(query.to, ts("2026-10-10T12:31:00Z"));
    }

    #[test]
    fn test_invalid_flow_id() {
        let source = json!({
            "flow_id": "abc",
            "timestamp": "2026-10-10T12:00:00.000000+0000",
        });
        assert_eq!(flow_query(&source), None);
    }
}
