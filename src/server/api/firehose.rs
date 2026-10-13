// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Live event stream. Every event the server ingests, whether read from
//! a file or socket or submitted by an agent, is published to the
//! firehose. Clients subscribe with optional server-side filters so a
//! "watch this IP" subscription stays usable on a busy sensor.

use crate::error::AppError;
use crate::prelude::*;
use crate::queryparser::{self, QueryElement, QueryValue};
use crate::server::{ServerContext, main::SessionExtractor};
use axum::Extension;
use axum::body::Body;
use axum::extract::Query;
use axum::response::sse::Event;
use axum::response::{IntoResponse, Sse};
use futures::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::error::TrySendError;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

/// Events queued per client between the firehose and the HTTP response.
/// A client that can't keep up has events dropped rather than queued
/// without bound; the drop is reported to the client as a `gap` event.
const CLIENT_QUEUE_SIZE: usize = 1024;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct FirehoseParams {
    /// Comma separated list of event types to include. All event types are
    /// sent when not provided or empty.
    pub event_type: Option<String>,

    /// Only include events from this sensor (the `host` field).
    pub sensor: Option<String>,

    /// An EveBox query string, like the search box in the other views.
    pub q: Option<String>,
}

impl FirehoseParams {
    fn matcher(&self) -> Result<FirehoseMatcher, AppError> {
        let event_types = self
            .event_type
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let sensor = self
            .sensor
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let query = match self.q.as_deref().map(|q| q.trim()) {
            Some(q) if !q.is_empty() => queryparser::parse(q, None)
                .map_err(|err| AppError::BadRequest(format!("q: {err:?}")))?,
            _ => vec![],
        };
        Ok(FirehoseMatcher {
            event_types,
            sensor,
            query,
        })
    }
}

/// Server-side filter for a firehose subscription.
#[derive(Debug, Default)]
struct FirehoseMatcher {
    event_types: Vec<String>,
    sensor: Option<String>,
    query: Vec<QueryElement>,
}

impl FirehoseMatcher {
    fn matches(&self, event: &serde_json::Value) -> bool {
        if !self.event_types.is_empty() {
            let matched = event["event_type"]
                .as_str()
                .map(|event_type| self.event_types.iter().any(|t| t == event_type))
                .unwrap_or(false);
            if !matched {
                return false;
            }
        }
        if let Some(sensor) = &self.sensor
            && event["host"].as_str() != Some(sensor.as_str())
        {
            return false;
        }
        if self.query.is_empty() {
            return true;
        }
        // Only serialized when a free text term needs it.
        let mut text: Option<String> = None;
        for element in &self.query {
            let matched = match &element.value {
                QueryValue::String(s) => {
                    let text = text.get_or_insert_with(|| event.to_string().to_lowercase());
                    text.contains(&s.to_lowercase())
                }
                QueryValue::KeyValue(k, v) => match k.as_str() {
                    "@ip" => field_equals(event, "src_ip", v) || field_equals(event, "dest_ip", v),
                    "@mac" => {
                        field_equals(event, "ether.src_mac", v)
                            || field_equals(event, "ether.dest_mac", v)
                    }
                    "@sid" => field_matches(event, "alert.signature_id", v),
                    "@sig" => field_matches(event, "alert.signature", v),
                    _ => field_matches(event, k, v),
                },
                QueryValue::Archived => crate::eve::eve::has_tag(event, "evebox.archived"),
                QueryValue::Escalated => crate::eve::eve::has_tag(event, "evebox.escalated"),
                // Time bounds don't apply to a live stream.
                QueryValue::From(_)
                | QueryValue::To(_)
                | QueryValue::After(_)
                | QueryValue::Before(_) => !element.negated,
            };
            if matched == element.negated {
                return false;
            }
        }
        true
    }
}

fn lookup<'a>(value: &'a serde_json::Value, path: &[&str]) -> Vec<&'a serde_json::Value> {
    match value {
        serde_json::Value::Array(values) => values.iter().flat_map(|v| lookup(v, path)).collect(),
        _ if path.is_empty() => vec![value],
        serde_json::Value::Object(values) => values
            .get(path[0])
            .map(|v| lookup(v, &path[1..]))
            .unwrap_or_default(),
        _ => vec![],
    }
}

/// Exact match against a dotted field, as a string.
fn field_equals(event: &serde_json::Value, field: &str, expected: &str) -> bool {
    let path: Vec<&str> = field.split('.').collect();
    lookup(event, &path).into_iter().any(|value| match value {
        serde_json::Value::String(s) => s == expected,
        serde_json::Value::Number(n) => n.to_string() == expected,
        serde_json::Value::Bool(b) => b.to_string() == expected,
        _ => false,
    })
}

/// Match a dotted field the way the datastores do for `key:value`: a
/// numeric value compares exactly, anything else is a case-insensitive
/// substring match.
fn field_matches(event: &serde_json::Value, field: &str, expected: &str) -> bool {
    let path: Vec<&str> = field.split('.').collect();
    let values = lookup(event, &path);
    if let Ok(n) = expected.parse::<i64>() {
        return values.iter().any(|value| match value {
            serde_json::Value::Number(number) => number.as_i64() == Some(n),
            serde_json::Value::String(s) => s == expected,
            _ => false,
        });
    }
    let expected = expected.to_lowercase();
    values.into_iter().any(|value| match value {
        serde_json::Value::String(s) => s.to_lowercase().contains(&expected),
        serde_json::Value::Bool(b) => b.to_string() == expected,
        _ => false,
    })
}

fn gap_event(dropped: u64) -> Option<Event> {
    Event::default()
        .event("gap")
        .json_data(serde_json::json!({ "dropped": dropped }))
        .ok()
}

pub(crate) async fn sse(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Query(params): Query<FirehoseParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let matcher = params.matcher()?;
    let mut firehose = context.firehose.subscribe();

    let (sse_tx, sse_rx) =
        tokio::sync::mpsc::channel::<Result<Event, Infallible>>(CLIENT_QUEUE_SIZE);
    let sse_stream = tokio_stream::wrappers::ReceiverStream::new(sse_rx);

    tokio::spawn(async move {
        // Events dropped since the client was last told about a gap,
        // either because the firehose lagged or the client queue was full.
        let mut dropped: u64 = 0;
        loop {
            let event = tokio::select! {
                // Leave as soon as the client disconnects, even if no
                // matching events arrive to reveal the closed response.
                _ = sse_tx.closed() => return,
                event = firehose.recv() => match event {
                    Ok(event) => event,
                    Err(RecvError::Lagged(n)) => {
                        dropped += n;
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                },
            };
            if !matcher.matches(&event) {
                continue;
            }
            if dropped > 0
                && let Some(gap) = gap_event(dropped)
            {
                match sse_tx.try_send(Ok(gap)) {
                    Ok(()) => dropped = 0,
                    Err(TrySendError::Full(_)) => {
                        dropped += 1;
                        continue;
                    }
                    Err(TrySendError::Closed(_)) => return,
                }
            }
            let event = match Event::default().json_data(event) {
                Ok(event) => event,
                Err(_) => continue,
            };
            match sse_tx.try_send(Ok(event)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => dropped += 1,
                Err(TrySendError::Closed(_)) => return,
            }
        }
    });

    Ok(Sse::new(sse_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    ))
}

pub(crate) async fn stream(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Query(params): Query<FirehoseParams>,
) -> Result<impl IntoResponse, AppError> {
    let matcher = params.matcher()?;
    let firehose: tokio::sync::broadcast::Receiver<serde_json::Value> =
        context.firehose.subscribe();

    let stream = BroadcastStream::new(firehose).filter_map(move |result| match result {
        Ok(value) => {
            if !matcher.matches(&value) {
                return None;
            }
            let mut string = value.to_string();
            string.push('\n');
            let bytes = axum::body::Bytes::from(string);
            Some(Ok::<bytes::Bytes, BroadcastStreamRecvError>(bytes))
        }
        Err(_) => None,
    });

    Ok(axum::response::Response::builder()
        .body(Body::from_stream(stream))
        .unwrap())
}

#[cfg(test)]
mod test {
    use super::*;

    fn matcher(event_type: Option<&str>, sensor: Option<&str>, q: Option<&str>) -> FirehoseMatcher {
        FirehoseParams {
            event_type: event_type.map(|s| s.to_string()),
            sensor: sensor.map(|s| s.to_string()),
            q: q.map(|s| s.to_string()),
        }
        .matcher()
        .unwrap()
    }

    fn alert() -> serde_json::Value {
        serde_json::json!({
            "event_type": "alert",
            "host": "sensor-a",
            "src_ip": "10.16.1.11",
            "dest_ip": "140.82.113.6",
            "ether": {"src_mac": "f0:9e:4a:3e:05:99"},
            "alert": {
                "signature": "ET INFO Session Traversal",
                "signature_id": 2016149,
                "severity": 3,
            },
            "tags": ["evebox.archived"],
            "dns": {"answers": [{"rrname": "example.com"}, {"rrname": "other.org"}]},
        })
    }

    #[test]
    fn event_type_filtering() {
        let alert = alert();
        let dns = serde_json::json!({"event_type": "dns"});
        let none = serde_json::json!({});

        let all = matcher(None, None, None);
        assert!(all.matches(&alert));
        assert!(all.matches(&none));

        let m = matcher(Some(" alert, dns ,"), None, None);
        assert_eq!(m.event_types, vec!["alert".to_string(), "dns".to_string()]);
        assert!(m.matches(&alert));
        assert!(m.matches(&dns));
        assert!(!m.matches(&none));

        assert!(!matcher(Some("dns"), None, None).matches(&alert));
    }

    #[test]
    fn sensor_filtering() {
        let alert = alert();
        assert!(matcher(None, Some("sensor-a"), None).matches(&alert));
        assert!(!matcher(None, Some("sensor-b"), None).matches(&alert));
        // Blank is the same as not set.
        assert!(matcher(None, Some(" "), None).matches(&alert));
        assert!(!matcher(None, Some("sensor-a"), None).matches(&serde_json::json!({})));
    }

    #[test]
    fn query_string_filtering() {
        let alert = alert();

        // Free text, case-insensitive, over the whole event.
        assert!(matcher(None, None, Some("traversal")).matches(&alert));
        assert!(matcher(None, None, Some("\"ET INFO\"")).matches(&alert));
        assert!(!matcher(None, None, Some("nomatch")).matches(&alert));
        assert!(!matcher(None, None, Some("-traversal")).matches(&alert));

        // key:value.
        assert!(matcher(None, None, Some("src_ip:10.16.1.11")).matches(&alert));
        assert!(!matcher(None, None, Some("src_ip:10.16.1.12")).matches(&alert));
        assert!(!matcher(None, None, Some("-src_ip:10.16.1.11")).matches(&alert));
        assert!(matcher(None, None, Some("alert.severity:3")).matches(&alert));
        assert!(!matcher(None, None, Some("alert.severity:1")).matches(&alert));
        assert!(matcher(None, None, Some("alert.signature:session")).matches(&alert));
        assert!(!matcher(None, None, Some("missing.field:x")).matches(&alert));

        // Arrays are searched.
        assert!(matcher(None, None, Some("dns.answers.rrname:other.org")).matches(&alert));

        // Special keys.
        assert!(matcher(None, None, Some("@ip:140.82.113.6")).matches(&alert));
        assert!(!matcher(None, None, Some("@ip:140.82.113.60")).matches(&alert));
        assert!(matcher(None, None, Some("@mac:f0:9e:4a:3e:05:99")).matches(&alert));
        assert!(matcher(None, None, Some("@sid:2016149")).matches(&alert));
        assert!(!matcher(None, None, Some("@sid:1")).matches(&alert));
        assert!(matcher(None, None, Some("@sig:traversal")).matches(&alert));
        assert!(matcher(None, None, Some("is:archived")).matches(&alert));
        assert!(!matcher(None, None, Some("is:escalated")).matches(&alert));
        assert!(matcher(None, None, Some("-is:escalated")).matches(&alert));

        // Multiple terms are ANDed, and combine with the other filters.
        assert!(
            matcher(
                Some("alert"),
                Some("sensor-a"),
                Some("src_ip:10.16.1.11 traversal")
            )
            .matches(&alert)
        );
        assert!(
            !matcher(
                Some("alert"),
                Some("sensor-a"),
                Some("src_ip:10.16.1.11 nomatch")
            )
            .matches(&alert)
        );

        // Time bounds are ignored for a live stream.
        assert!(matcher(None, None, Some("@from:2024")).matches(&alert));
    }
}
