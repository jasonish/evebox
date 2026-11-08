// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Live event stream, fed by the server's live feed: events as the
//! server ingests them, read from a file or socket or submitted by an
//! agent, or, with Elasticsearch or OpenSearch and nothing ingested
//! through EveBox, events polled from the datastore. Clients subscribe
//! with optional server-side filters so a "watch this IP" subscription
//! stays usable on a busy sensor.

use crate::error::AppError;
use crate::prelude::*;
use crate::queryparser::{self, QueryElement, QueryValue};
use crate::server::livefeed::{FeedStatus, Subscription};
use crate::server::{ServerContext, main::SessionExtractor};
use axum::Extension;
use axum::body::Body;
use axum::extract::Query;
use axum::http::header;
use axum::response::sse::Event;
use axum::response::{IntoResponse, Sse};
use futures::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

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

/// The sensor filter value for events without a sensor.
const NO_SENSOR: &str = "(no-name)";

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
        if let Some(sensor) = &self.sensor {
            let identity = crate::server::routing::sensor_identity(event);
            // As the sensor list names events without a sensor.
            let matched = if sensor == NO_SENSOR {
                identity.is_none()
            } else {
                identity == Some(sensor.as_str())
            };
            if !matched {
                return false;
            }
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

/// The feed's status, as a named event so it is never mistaken for an
/// EVE event.
fn status_event(status: &FeedStatus) -> Option<Event> {
    Event::default().event("status").json_data(status).ok()
}

type SseSender = mpsc::Sender<Result<Event, Infallible>>;

/// Server-sent events: the feed's status first, then matching events, and
/// the status again whenever it changes. Events dropped because the client
/// or the server couldn't keep up are reported as a `gap` event with the
/// number dropped.
pub(crate) async fn sse(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Query(params): Query<FirehoseParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let matcher = params.matcher()?;
    let subscription = context.live_feed.subscribe();
    let (sse_tx, sse_rx) = mpsc::channel(CLIENT_QUEUE_SIZE);
    tokio::spawn(forward_sse(subscription, matcher, sse_tx));
    Ok(
        Sse::new(tokio_stream::wrappers::ReceiverStream::new(sse_rx)).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(1))
                .text("keep-alive-text"),
        ),
    )
}

/// Forward the feed to one client until it goes away. The subscription
/// is held here, so it ends with the client even while no events arrive.
async fn forward_sse(mut subscription: Subscription, matcher: FirehoseMatcher, tx: SseSender) {
    // The client queue is still empty, so this always fits.
    let status = subscription.status.borrow_and_update().clone();
    if let Some(event) = status_event(&status)
        && tx.try_send(Ok(event)).is_err()
    {
        return;
    }
    // A status change that didn't fit in the client queue, to be sent as
    // soon as there's room. Events can be dropped, but the client must
    // not be left with a stale status.
    let mut status_pending = false;
    // Events dropped since the client was last told about a gap, either
    // because the feed lagged or the client queue was full.
    let mut dropped: u64 = 0;
    loop {
        tokio::select! {
            // Leave as soon as the client disconnects, even if no
            // matching events arrive to reveal the closed response.
            _ = tx.closed() => return,
            permit = tx.reserve(), if status_pending => {
                let Ok(permit) = permit else { return };
                let status = subscription.status.borrow_and_update().clone();
                if let Some(event) = status_event(&status) {
                    permit.send(Ok(event));
                }
                status_pending = false;
            }
            changed = subscription.status.changed() => {
                if changed.is_err() {
                    return;
                }
                let status = subscription.status.borrow_and_update().clone();
                if let Some(event) = status_event(&status) {
                    match tx.try_send(Ok(event)) {
                        Ok(()) => status_pending = false,
                        Err(TrySendError::Full(_)) => status_pending = true,
                        Err(TrySendError::Closed(_)) => return,
                    }
                }
            }
            event = subscription.events.recv() => {
                let event = match event {
                    Ok(event) => event,
                    Err(RecvError::Lagged(n)) => {
                        dropped += n;
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                };
                if !matcher.matches(&event) {
                    continue;
                }
                if dropped > 0
                    && let Some(gap) = gap_event(dropped)
                {
                    match tx.try_send(Ok(gap)) {
                        Ok(()) => dropped = 0,
                        Err(TrySendError::Full(_)) => {
                            dropped += 1;
                            continue;
                        }
                        Err(TrySendError::Closed(_)) => return,
                    }
                }
                let Ok(event) = Event::default().json_data(&*event) else {
                    continue;
                };
                match tx.try_send(Ok(event)) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => dropped += 1,
                    Err(TrySendError::Closed(_)) => return,
                }
            }
        }
    }
}

/// Newline delimited JSON: one matching event per line, nothing else.
/// Events the client couldn't keep up with are dropped silently, as this
/// format has nowhere to report them.
pub(crate) async fn stream(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Query(params): Query<FirehoseParams>,
) -> Result<impl IntoResponse, AppError> {
    let matcher = params.matcher()?;
    // Held by the response body, so it ends when the body is dropped.
    let subscription = context.live_feed.subscribe();
    let stream = futures::stream::unfold(
        (subscription, matcher),
        |(mut subscription, matcher)| async move {
            loop {
                match subscription.events.recv().await {
                    Ok(event) => {
                        if matcher.matches(&event) {
                            let mut line = event.to_string();
                            line.push('\n');
                            let line = axum::body::Bytes::from(line);
                            return Some((Ok::<_, Infallible>(line), (subscription, matcher)));
                        }
                    }
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return None,
                }
            }
        },
    );

    Ok(axum::response::Response::builder()
        .header(header::CONTENT_TYPE, "application/x-ndjson")
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

    #[test]
    fn sensor_filtering_uses_the_sensor_identity() {
        // ECS shaped: the sensor is agent.name, host is an object.
        let ecs = serde_json::json!({
            "host": {"name": "os-hostname"},
            "agent": {"name": "sensor-a"},
        });
        assert!(matcher(None, Some("sensor-a"), None).matches(&ecs));
        assert!(!matcher(None, Some("os-hostname"), None).matches(&ecs));
    }

    #[test]
    fn the_no_name_sensor_matches_events_without_a_sensor() {
        let no_name = matcher(None, Some("(no-name)"), None);
        assert!(no_name.matches(&serde_json::json!({"event_type": "alert"})));
        assert!(!no_name.matches(&alert()));
    }

    /// The endpoints over HTTP.
    mod http {
        use super::*;
        use crate::eventrepo::EventRepo;
        use crate::server::ServerConfig;
        use crate::server::livefeed::LiveFeed;
        use crate::server::livefeed::testing::*;
        use crate::server::main::build_axum_service;
        use crate::server::metrics::Metrics;
        use crate::sqlite::connection::{ConnectionBuilder, init_event_db};
        use crate::sqlite::eventrepo::SqliteEventRepo;
        use futures::StreamExt;
        use serde_json::json;
        use std::net::SocketAddr;
        use std::pin::Pin;
        use std::sync::atomic::Ordering;
        use tokio::io::AsyncWriteExt;

        /// Serve the API with `feed` as its live feed, or with a SQLite
        /// server's own feed of ingested events.
        async fn serve(
            feed: Option<LiveFeed>,
        ) -> (
            SocketAddr,
            tokio::task::JoinHandle<()>,
            tempfile::TempDir,
            Arc<ServerContext>,
        ) {
            let _ = rustls::crypto::ring::default_provider().install_default();
            let dir = tempfile::tempdir().unwrap();
            let builder = ConnectionBuilder::filename(Some(&dir.path().join("events.sqlite")));
            let mut writer = builder.open_connection(true).await.unwrap();
            init_event_db(&mut writer).await.unwrap();
            let pool = builder.open_pool(false).await.unwrap();
            let datastore = EventRepo::SQLite(SqliteEventRepo::new(
                Arc::new(tokio::sync::Mutex::new(writer)),
                pool,
                Arc::new(Metrics::default()),
            ));
            let configdb = crate::sqlite::configdb::open(Some(&dir.path().join("config.sqlite")))
                .await
                .unwrap();
            let mut context = ServerContext::new(
                ServerConfig::default(),
                Arc::new(configdb),
                datastore,
                Arc::new(Metrics::default()),
            );
            if let Some(feed) = feed {
                context.live_feed = feed;
            }
            let context = Arc::new(context);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let service = build_axum_service(context.clone());
            let server = tokio::spawn(async move {
                axum::serve(listener, service).await.unwrap();
            });
            (address, server, dir, context)
        }

        type BodyStream =
            Pin<Box<dyn futures::Stream<Item = reqwest::Result<axum::body::Bytes>> + Send>>;

        /// Reads server-sent events.
        struct SseReader {
            body: BodyStream,
            buffer: String,
        }

        impl SseReader {
            async fn connect(address: SocketAddr, query: &str) -> Self {
                let response = reqwest::Client::new()
                    .get(format!("http://{address}/api/firehose/sse{query}"))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                Self {
                    body: Box::pin(response.bytes_stream()),
                    buffer: String::new(),
                }
            }

            /// The next event's name ("message" if unnamed) and data,
            /// skipping keep-alive comments.
            async fn next(&mut self) -> (String, serde_json::Value) {
                loop {
                    if let Some(end) = self.buffer.find("\n\n") {
                        let block: String = self.buffer.drain(..end + 2).collect();
                        let mut name = "message".to_string();
                        let mut data = None;
                        for line in block.lines() {
                            if let Some(value) = line.strip_prefix("event:") {
                                name = value.trim().to_string();
                            } else if let Some(value) = line.strip_prefix("data:") {
                                data = Some(value.trim_start().to_string());
                            }
                        }
                        if let Some(data) = data {
                            return (name, serde_json::from_str(&data).unwrap());
                        }
                        continue;
                    }
                    let chunk = tokio::time::timeout(WAIT, self.body.next())
                        .await
                        .expect("timed out waiting for an event")
                        .expect("the stream ended")
                        .unwrap();
                    self.buffer.push_str(std::str::from_utf8(&chunk).unwrap());
                }
            }

            /// The next status, skipping any until `want` holds.
            async fn status_until(
                &mut self,
                want: impl Fn(&serde_json::Value) -> bool,
            ) -> serde_json::Value {
                loop {
                    let (name, data) = self.next().await;
                    assert_eq!(name, "status", "expected a status, got {data}");
                    if want(&data) {
                        return data;
                    }
                }
            }

            async fn next_event(&mut self) -> serde_json::Value {
                loop {
                    let (name, data) = self.next().await;
                    if name == "message" {
                        return data;
                    }
                }
            }
        }

        fn is_ok(status: &serde_json::Value) -> bool {
            status["state"] == "ok"
        }

        #[tokio::test]
        async fn sse_sends_the_status_first_then_events() {
            let (address, server, _dir, context) = serve(None).await;
            let mut sse = SseReader::connect(address, "").await;
            let (name, status) = sse.next().await;
            assert_eq!(name, "status");
            assert_eq!(
                status,
                json!({"source": "ingest", "state": "ok", "behind": false})
            );
            context
                .live_feed
                .publish_ingested(&json!({"event_type": "alert", "id": "a"}));
            assert_eq!(
                sse.next().await,
                (
                    "message".to_string(),
                    json!({"event_type": "alert", "id": "a"})
                )
            );
            server.abort();
        }

        #[tokio::test]
        async fn sse_reports_polling_and_sends_polled_events() {
            let harness = Harness::new();
            harness.source.add("i", "old", NOW - 1_000);
            let (address, server, _dir, _context) = serve(Some(harness.feed.clone())).await;
            let mut sse = SseReader::connect(address, "").await;
            let (name, status) = sse.next().await;
            assert_eq!(name, "status");
            assert_eq!(status["source"], "datastore");
            if status["state"] != "ok" {
                assert_eq!(status["state"], "starting");
                sse.status_until(is_ok).await;
            }
            harness.source.add("i", "new", NOW);
            assert_eq!(sse.next_event().await["id"], "new");
            server.abort();
        }

        #[tokio::test]
        async fn an_empty_submission_doesnt_stop_polling() {
            let harness = Harness::new();
            let (address, server, _dir, _context) = serve(Some(harness.feed.clone())).await;
            let mut sse = SseReader::connect(address, "").await;
            sse.status_until(is_ok).await;
            // As the Go agent has been seen to send, or anyone, as
            // submitting needs no login.
            let response = reqwest::Client::new()
                .post(format!("http://{address}/api/submit"))
                .body("")
                .send()
                .await
                .unwrap();
            let response: serde_json::Value = response.json().await.unwrap();
            assert_eq!(response["Count"], 0);
            // Not taken for ingest, which would have started polling over
            // later, with this in the new baseline.
            harness.source.add("i", "new", NOW);
            assert_eq!(sse.next_event().await["id"], "new");
            server.abort();
        }

        #[tokio::test]
        async fn filters_apply_per_client_to_one_shared_poll() {
            let harness = Harness::new();
            let (address, server, _dir, _context) = serve(Some(harness.feed.clone())).await;
            let mut alerts = SseReader::connect(address, "?event_type=alert").await;
            let mut dns = SseReader::connect(address, "?event_type=dns").await;
            alerts.status_until(is_ok).await;
            dns.status_until(is_ok).await;
            assert_eq!(harness.feed.subscribers(), 2);
            assert_eq!(harness.stats().tasks.load(Ordering::SeqCst), 1);

            for (id, event_type) in [
                ("a1", "alert"),
                ("d1", "dns"),
                ("a2", "alert"),
                ("d2", "dns"),
            ] {
                harness
                    .source
                    .add_event("i", id, NOW, json!({"event_type": event_type, "id": id}));
            }
            let mut got_alerts = vec![
                alerts.next_event().await["id"].clone(),
                alerts.next_event().await["id"].clone(),
            ];
            let mut got_dns = vec![
                dns.next_event().await["id"].clone(),
                dns.next_event().await["id"].clone(),
            ];
            got_alerts.sort_by_key(|id| id.to_string());
            got_dns.sort_by_key(|id| id.to_string());
            assert_eq!(got_alerts, vec![json!("a1"), json!("a2")]);
            assert_eq!(got_dns, vec![json!("d1"), json!("d2")]);
            server.abort();
        }

        #[tokio::test]
        async fn an_idle_sse_client_leaving_ends_its_subscription() {
            let harness = Harness::new();
            let (address, server, _dir, _context) = serve(Some(harness.feed.clone())).await;
            let mut sse = SseReader::connect(address, "").await;
            sse.status_until(is_ok).await;
            assert_eq!(harness.feed.subscribers(), 1);
            drop(sse);
            wait_for(|| harness.feed.subscribers() == 0).await;
            wait_for(|| harness.source.in_flight.load(Ordering::SeqCst) == 0).await;
            let scans = harness.source.scans();
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(harness.source.scans(), scans);
            server.abort();
        }

        #[tokio::test]
        async fn ndjson_sends_one_event_per_line() {
            let harness = Harness::new();
            let (address, server, _dir, _context) = serve(Some(harness.feed.clone())).await;
            let response = reqwest::Client::new()
                .get(format!("http://{address}/api/firehose?event_type=alert"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/x-ndjson"
            );
            let mut body = response.bytes_stream();
            wait_for(|| harness.stats().baselines.load(Ordering::SeqCst) == 1).await;
            harness
                .source
                .add_event("i", "d", NOW, json!({"event_type": "dns", "id": "d"}));
            harness
                .source
                .add_event("i", "a", NOW, json!({"event_type": "alert", "id": "a"}));
            let mut text = String::new();
            while !text.contains('\n') {
                let chunk = tokio::time::timeout(WAIT, body.next())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                text.push_str(std::str::from_utf8(&chunk).unwrap());
            }
            assert_eq!(text, "{\"event_type\":\"alert\",\"id\":\"a\"}\n");
            drop(body);
            wait_for(|| harness.feed.subscribers() == 0).await;
            server.abort();
        }

        #[tokio::test]
        async fn an_idle_ndjson_client_leaving_ends_its_subscription() {
            let harness = Harness::new();
            let (address, server, _dir, _context) = serve(Some(harness.feed.clone())).await;
            // A plain connection that reads nothing, so the only sign of
            // the client leaving is the connection closing.
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream
                .write_all(b"GET /api/firehose HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
            wait_for(|| harness.feed.subscribers() == 1).await;
            wait_for(|| harness.stats().baselines.load(Ordering::SeqCst) == 1).await;
            drop(stream);
            wait_for(|| harness.feed.subscribers() == 0).await;
            server.abort();
        }

        #[tokio::test]
        async fn sse_and_ndjson_clients_share_one_poll() {
            let harness = Harness::new();
            let (address, server, _dir, _context) = serve(Some(harness.feed.clone())).await;
            let mut sse = SseReader::connect(address, "").await;
            sse.status_until(is_ok).await;
            let response = reqwest::Client::new()
                .get(format!("http://{address}/api/firehose"))
                .send()
                .await
                .unwrap();
            let mut ndjson = response.bytes_stream();
            wait_for(|| harness.feed.subscribers() == 2).await;
            harness.source.add("i", "shared", NOW);
            assert_eq!(sse.next_event().await["id"], "shared");
            let chunk = tokio::time::timeout(WAIT, ndjson.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(std::str::from_utf8(&chunk).unwrap().contains("\"shared\""));
            // One period, one baseline, the event fetched once.
            assert_eq!(harness.stats().tasks.load(Ordering::SeqCst), 1);
            assert_eq!(harness.stats().baselines.load(Ordering::SeqCst), 1);
            assert_eq!(harness.stats().emitted.load(Ordering::SeqCst), 1);
            server.abort();
        }
    }
}
