// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Full packet capture API: `POST /api/pcap` extracts packets from
//! a server-local capture source or a connected agent.

use std::collections::VecDeque;
#[cfg(not(windows))]
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
#[cfg(not(windows))]
use std::sync::atomic::AtomicU64;
#[cfg(not(windows))]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use axum::body::Body;
use axum::extract::{ConnectInfo, Extension, Json, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::prelude::*;
use bytes::Bytes;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent::protocol::{
    CONTROL_MESSAGE_MAX_BYTES, PcapResult, PcapResultCode, PcapUploadStatus, ServerMessage,
    WireLimits, WirePcapFilter,
};
#[cfg(all(test, not(windows)))]
use crate::pcap::SpoolConfig;
use crate::pcap::{self, FetchStats, FlowSelector, Limits, PcapFilter, PcapRequest};
#[cfg(not(windows))]
use crate::pcap::{FetchError, PcapSource};
use crate::prelude::*;
use crate::server::ServerContext;
use crate::server::agents::AgentEntry;
use crate::server::main::SessionExtractor;
use crate::server::pcap::tasks::{self, RemoteOutcome, UploadState};
use crate::server::pcap::{PcapRouting, ResolvedPcapSource, RouteError};

/// Chunk size streamed to the client; the writer buffers extraction
/// output up to this before pushing a frame.
#[cfg(not(windows))]
const CHUNK_SIZE: usize = 64 * 1024;

static PCAP_CONTENT_TYPE: HeaderValue =
    HeaderValue::from_static(crate::agent::protocol::PCAP_CONTENT_TYPE);

/// The `POST /api/pcap` request body. All fields are optional; the
/// combination present selects the mode (see [`build_request`]). Old
/// clients sending only `{event_id}` keep the default auto-derive
/// behavior.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PcapRequestBody {
    /// The event to build the capture from. Absent for a standalone
    /// (no-event) request.
    #[serde(default)]
    pub event_id: Option<String>,
    /// Raw libpcap BPF. Non-empty => that filter; empty `""` => all
    /// packets in the window; absent => the event's derived flow
    /// (event-relative / default) or all packets (free-form).
    #[serde(default)]
    pub filter: Option<String>,
    /// RFC3339 absolute start; its presence selects free-form mode.
    #[serde(default)]
    pub start: Option<String>,
    /// `1m`/`5m`/`1h` span; with `start` gives `[start, start+dur]`.
    #[serde(default)]
    pub duration: Option<String>,
    /// `1m` style; event-relative window mode (needs `event_id`).
    #[serde(default)]
    pub before: Option<String>,
    /// `1m` style; event-relative window mode (needs `event_id`).
    #[serde(default)]
    pub after: Option<String>,
    /// Per-request output cap (`50mb`, `2gb`, or a bare byte count).
    /// Native GET may raise or lift the fixed server default;
    /// `0`/`none`/`unlimited` removes its cap. Buffered POST may only
    /// keep or lower the default so its in-memory response stays
    /// bounded. Absent keeps the server default.
    #[serde(default)]
    pub max_size: Option<String>,
    /// Optional pcap source name. Event requests normally route by
    /// sensor identity; standalone requests use this when more than
    /// one local/remote source is available.
    #[serde(default)]
    pub source: Option<String>,
}

/// `POST /api/pcap`: bounded, buffered quick extraction for an event's flow.
pub(crate) async fn post_pcap(
    State(context): State<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Extension(ConnectInfo(remote)): Extension<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(body): Json<PcapRequestBody>,
) -> Response {
    let user = session.username.clone().unwrap_or_else(|| "-".to_string());
    let remote = remote_addr(&context, &headers, remote);

    // `native = false`: the POST caller buffers the response and reads
    // its body, so an empty result stays a structured JSON error it can
    // surface as a message.
    let buffer_limit = context.pcap.settings.max_bytes;
    let response = match handle(&context, &body, &user, remote, false).await {
        Ok(response) | Err(response) => response,
    };
    buffer_post_body(response, buffer_limit).await
}

/// `GET /api/pcap`: the native-download form of `POST /api/pcap`. The
/// browser navigates here so the capture streams straight to disk with
/// no in-memory buffering; the request parameters arrive as the query
/// string. A malformed request is normally caught first by the
/// `GET /api/pcap/validate` pre-flight, so this streams on the happy
/// path but still returns the same structured errors directly.
pub(crate) async fn get_pcap(
    State(context): State<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Extension(ConnectInfo(remote)): Extension<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Query(body): Query<PcapRequestBody>,
) -> Response {
    let user = session.username.clone().unwrap_or_else(|| "-".to_string());
    let remote = remote_addr(&context, &headers, remote);
    // `native = true`: the browser streams this straight to disk and
    // never reads the response, so an empty result becomes a valid
    // empty pcap rather than a JSON error saved under a `.pcap` name.
    match handle(&context, &body, &user, remote, true).await {
        Ok(response) | Err(response) => response,
    }
}

/// `GET /api/pcap/sources`: the pcap source names a request's `source`
/// parameter may select right now — the local source (when configured) and
/// the connected pcap-capable agents. The webapp's custom download form
/// offers these in its source picker.
pub(crate) async fn get_sources(
    State(context): State<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Response {
    Json(list_sources(&context)).into_response()
}

fn list_sources(context: &ServerContext) -> serde_json::Value {
    #[derive(Serialize)]
    struct Source {
        name: String,
        kind: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        hostname: Option<String>,
    }
    let mut sources = Vec::new();
    if context.pcap.has_source() {
        sources.push(Source {
            name: crate::server::agents::LOCAL_PCAP_SOURCE_NAME.to_string(),
            kind: "server",
            hostname: None,
        });
    }
    sources.extend(
        context
            .agents
            .pcap_agents()
            .into_iter()
            .map(|agent| Source {
                name: agent.name.clone(),
                kind: "agent",
                hostname: Some(agent.hostname.clone()),
            }),
    );
    serde_json::json!({ "sources": sources })
}

/// `GET /api/pcap/routing`: the operator pcap routing table. Same
/// sensitivity as `GET /api/pcap/sources` — the event view mirrors it
/// to grey the PCAP button.
pub(crate) async fn get_routing(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
) -> impl IntoResponse {
    Json(context.pcap.get_routing())
}

/// Generous bounds on the routing table: the table is cloned per lookup-free
/// GET, fetched by the event view, and scanned per pcap request. Rule count is
/// bounded for that reason; names share the deliberately loose agent limit.
const ROUTING_MAX_RULES: usize = 256;
const ROUTING_MAX_NAME: usize = crate::server::agents::MAX_AGENT_NAME_BYTES;

/// `POST /api/pcap/routing`: validate, persist, and apply the operator
/// pcap routing table, taking effect without a restart.
pub(crate) async fn post_routing(
    _session: SessionExtractor,
    State(context): State<Arc<ServerContext>>,
    Json(mut routing): Json<PcapRouting>,
) -> Result<impl IntoResponse, AppError> {
    if routing.rules.len() > ROUTING_MAX_RULES {
        return Err(AppError::BadRequest(format!(
            "the routing table is limited to {ROUTING_MAX_RULES} rules"
        )));
    }
    for rule in &mut routing.rules {
        rule.sensor = rule.sensor.trim().to_string();
        rule.source = rule.source.trim().to_string();
        if rule.sensor.is_empty() || rule.source.is_empty() {
            return Err(AppError::BadRequest(
                "every routing rule requires a sensor and a source".to_string(),
            ));
        }
        if rule.sensor.len() > ROUTING_MAX_NAME || rule.source.len() > ROUTING_MAX_NAME {
            return Err(AppError::BadRequest(format!(
                "sensor and source names are limited to {ROUTING_MAX_NAME} bytes"
            )));
        }
    }
    if let Some(default) = &routing.default {
        let default = default.trim();
        if default.is_empty() {
            return Err(AppError::BadRequest(
                "the default source must not be empty".to_string(),
            ));
        }
        if default.len() > ROUTING_MAX_NAME {
            return Err(AppError::BadRequest(format!(
                "sensor and source names are limited to {ROUTING_MAX_NAME} bytes"
            )));
        }
        routing.default = Some(default.to_string());
    }
    // Persist and apply under one lock: concurrent saves could
    // otherwise interleave, leaving the configdb with one table and
    // the live service with another until the next save or restart.
    let _lock = context.pcap.routing_save.lock().await;
    context
        .configdb
        .kv_set_config("config.pcap.routing", &serde_json::to_value(&routing)?)
        .await?;
    context.pcap.set_routing(routing);
    Ok(Json(serde_json::json!({})))
}

/// `GET /api/pcap/validate`: pre-flight for the native download. Runs
/// the same event load and filter/window/max-size
/// validation as a real request but takes no permit and extracts
/// nothing, returning `{ok, filename}` on success or
/// the same structured error a real request would.
pub(crate) async fn validate_pcap(
    State(context): State<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Extension(ConnectInfo(remote)): Extension<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Query(body): Query<PcapRequestBody>,
) -> Response {
    let user = session.username.clone().unwrap_or_else(|| "-".to_string());
    let remote = remote_addr(&context, &headers, remote);
    match handle_inner(&context, &body, &user, remote, true, false).await {
        Ok(response) | Err(response) => response,
    }
}

/// The client address for the audit log: `x-forwarded-for` when
/// reverse-proxy support is enabled, else the socket peer.
fn remote_addr(context: &ServerContext, headers: &HeaderMap, remote: SocketAddr) -> String {
    if context.config.http_reverse_proxy
        && let Some(forwarded) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
    {
        return forwarded.to_string();
    }
    remote.to_string()
}

/// Extract-and-stream entry point (real download): validate, take
/// permits, and stream the packets. Used by `POST`/`GET /api/pcap`.
async fn handle(
    context: &Arc<ServerContext>,
    body: &PcapRequestBody,
    user: &str,
    remote: String,
    native: bool,
) -> Result<Response, Response> {
    handle_inner(context, body, user, remote, false, native).await
}

/// Shared request handling. With `dry_run` it stops after structural
/// validation (event loaded, capture source resolved, window and max-size
/// checked) and returns a 200 JSON summary without taking a permit or spawning
/// extraction. Filter compilation and capture I/O remain on the serving side;
/// native-download errors are read from the hidden browser frame.
async fn handle_inner(
    context: &Arc<ServerContext>,
    body: &PcapRequestBody,
    user: &str,
    remote: String,
    dry_run: bool,
    native: bool,
) -> Result<Response, Response> {
    // The audit line fields, filled in as the request is understood.
    // Every outcome — success or failure, however early — logs
    // exactly one `pcap:` line built from these. A standalone request
    // (no event) carries event="-".
    let mut audit = AuditContext {
        user: user.to_string(),
        remote,
        event_id: body.event_id.clone().unwrap_or_else(|| "-".to_string()),
        mode: "-",
        filter: "-".to_string(),
        window: "-".to_string(),
        source: "-".to_string(),
        native,
    };

    let settings = &context.pcap.settings;

    // Load the event only when an event_id is given: needed for event
    // context, the event-relative window, and default mode.
    // Standalone requests never touch the datastore.
    let event = match &body.event_id {
        Some(event_id) => match context.datastore.get_event_by_id(event_id.clone()).await {
            Ok(Some(event)) => Some(event),
            Ok(None) => {
                return Err(fail(
                    &audit,
                    StatusCode::NOT_FOUND,
                    "event-not-found",
                    "event not found",
                ));
            }
            Err(err) => {
                error!(
                    "Pcap request failed to load event {:?}: {}",
                    body.event_id, err
                );
                return Err(fail(
                    &audit,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "failed to load event",
                ));
            }
        },
        None => None,
    };
    let event_source = event.as_ref().map(|event| &event["_source"]);

    // WINDOW + FILTER: build the engine filter and time window from
    // the request mode, so a bad time or duration fails before any
    // permit is taken or extraction spawned. A supplied BPF filter is
    // passed through unchecked: the compiling side — the local engine
    // or the serving agent — rejects a malformed expression and that
    // error propagates through the download result.
    let (filter, window) = match build_request(&mut audit, body, event_source) {
        Ok(pair) => pair,
        Err(err) => return Err(fail(&audit, err.status, err.code, &err.message)),
    };

    // Per-request output cap: the custom builder / standalone native
    // download may raise (or lift) the fixed server default for one
    // request. A bad value fails up front, before any permit is taken.
    let max_bytes = match present(&body.max_size) {
        Some(raw) => match parse_max_bytes(raw) {
            Ok(bytes) => bytes,
            Err(err) => {
                return Err(fail(
                    &audit,
                    StatusCode::BAD_REQUEST,
                    "bad-max-size",
                    &format!("bad max-size: {err}"),
                ));
            }
        },
        None => settings.max_bytes,
    };

    // POST is the buffered quick-download API. Keep its aggregation
    // bounded by the fixed server default; callers that need a larger
    // or unlimited response must use the native GET path, which streams
    // directly to the browser. The dry-run preflight remains permissive
    // because it validates requests for that GET path.
    if !dry_run && !native && max_bytes > settings.max_bytes {
        return Err(fail(
            &audit,
            StatusCode::PAYLOAD_TOO_LARGE,
            "max-size-too-large",
            "buffered pcap downloads cannot exceed the server default; use GET /api/pcap for larger downloads",
        ));
    }

    // Choose the configured local source or a connected pcap-capable agent only
    // after the request has been normalized. Both producers receive the exact
    // same effective filter, bounds, and limits.
    let source =
        match context
            .pcap
            .resolve_source(&context.agents, event_source, present(&body.source))
        {
            Ok(source) => source,
            Err(RouteError::NoSource(sensor)) => {
                let message = sensor
                    .as_deref()
                    .map(|sensor| format!("no pcap source connected for {sensor}"))
                    .unwrap_or_else(|| "no pcap source is configured or connected".to_string());
                return Err(fail(
                    &audit,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "no-source",
                    &message,
                ));
            }
            Err(RouteError::NoRule(sensor)) => {
                // Distinct from NoSource: naming the sensor as a
                // "source" would send the operator connecting an agent
                // by that name, which the table would still ignore.
                let message = sensor
                    .as_deref()
                    .map(|sensor| format!("no pcap routing rule matches sensor {sensor}"))
                    .unwrap_or_else(|| {
                        "no pcap routing rule matches this request and no default source is set"
                            .to_string()
                    });
                return Err(fail(
                    &audit,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "no-source",
                    &message,
                ));
            }
            Err(RouteError::Ambiguous(candidates)) => {
                let response = serde_json::json!({
                    "error": {
                        "code": "ambiguous-source",
                        "message": "multiple pcap sources could serve this request",
                        "candidates": candidates,
                    }
                });
                warn!(
                    "pcap: user={:?} remote={:?} event={:?} outcome=ambiguous-source",
                    audit.user, audit.remote, audit.event_id
                );
                return Err((StatusCode::CONFLICT, Json(response)).into_response());
            }
        };
    audit.source = source.name().to_string();

    let filename = filename(event_source, window.start.to_seconds());

    // Pre-flight (native-download probe): event, source, window and max-size
    // are valid, so report success without taking a permit or spawning
    // extraction. BPF compilation remains on the local engine or serving
    // agent; a failure is returned as structured JSON to the browser frame.
    if dry_run {
        return Ok(axum::Json(serde_json::json!({
            "ok": true,
            "filename": filename,
        }))
        .into_response());
    }
    // Concurrency is effectively unbounded, but a single global slot
    // still backstops the shared blocking pool. The slot doubles as
    // the supervisor's release signal when the request settles.
    let Some(global_permit) = context.pcap.try_acquire() else {
        return Err(fail(
            &audit,
            StatusCode::TOO_MANY_REQUESTS,
            "busy",
            "too many concurrent pcap requests",
        ));
    };

    // The engine takes the built filter (a derived flow, a raw BPF
    // expression, or None for all packets) and the window bounds as
    // unix microseconds; the request timeout doubles as the
    // engine-side wall-clock scan cap.
    let request = PcapRequest {
        filter,
        start: Some(to_micros(&window.start)),
        end: Some(to_micros(&window.end)),
        limits: Limits {
            max_bytes,
            deadline: Some(settings.request_timeout),
            ..Limits::default()
        },
    };

    let Some(source_permit) = source.try_acquire() else {
        return Err(fail(
            &audit,
            StatusCode::TOO_MANY_REQUESTS,
            "source-busy",
            "pcap source is busy",
        ));
    };

    match source {
        // A Windows server cannot register a local spool, so a Local
        // resolution is unreachable there; extraction is agent-only.
        #[cfg(windows)]
        ResolvedPcapSource::Local { .. } => Err(fail(
            &audit,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "server-local pcap capture is not supported on this platform",
        )),
        #[cfg(not(windows))]
        ResolvedPcapSource::Local { source, .. } => {
            // A wedged local source can accumulate detached extraction threads.
            let backlog = context.pcap.inflight.load(Ordering::SeqCst);
            if backlog >= settings.max_concurrent.saturating_mul(2).max(2) {
                return Err(fail(
                    &audit,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "wedged",
                    "pcap extraction backlog; capture source may be unresponsive",
                ));
            }
            stream_local(
                context.clone(),
                source,
                request,
                filename,
                audit,
                global_permit,
                source_permit,
            )
            .await
        }
        ResolvedPcapSource::Agent(entry) => {
            stream_agent(
                context.clone(),
                entry,
                request,
                filename,
                audit,
                global_permit,
                source_permit,
            )
            .await
        }
    }
}

/// A window bound as unix microseconds for the engine; pre-epoch
/// times clamp to zero.
fn to_micros(datetime: &crate::datetime::DateTime) -> u64 {
    u64::try_from(datetime.datetime.timestamp_micros()).unwrap_or(0)
}

/// Fields for the single audit log line emitted per request.
struct AuditContext {
    user: String,
    remote: String,
    /// The event id, or "-" for a standalone request.
    event_id: String,
    /// The resolved request mode: `default`, `relative`, `freeform`,
    /// or "-" before it is known.
    mode: &'static str,
    /// The effective filter: a flow selector description, the raw BPF
    /// string, or `all` for a match-all window.
    filter: String,
    /// The effective time window, `start..end`, or "-" before known.
    window: String,
    /// The resolved local spool or connected agent name.
    source: String,
    /// The browser native-download form (`GET /api/pcap`): the client
    /// cannot read an error body, so an empty result becomes a valid,
    /// openable empty pcap instead of a JSON error saved as a `.pcap`.
    native: bool,
}

/// Response headers for a successful pcap download.
fn pcap_headers(audit: &AuditContext, filename: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, PCAP_CONTENT_TYPE.clone());
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename={filename}")) {
        headers.insert(CONTENT_DISPOSITION, value);
    }
    if audit.source != "-"
        && let Ok(value) = HeaderValue::from_str(&audit.source)
    {
        headers.insert(HeaderName::from_static("x-evebox-pcap-source"), value);
    }
    headers
}

/// Buffer the POST response until its body observes the producer's terminal
/// frame. The browser POST client buffers the body as a Blob anyway, and
/// waiting here lets a multi-chunk capture report limit truncation before the
/// HTTP headers are committed. Native GET downloads keep their direct-to-disk
/// streaming path.
async fn buffer_post_body(response: Response, max_bytes: u64) -> Response {
    let Some(completion) = response.extensions().get::<PostCompletion>().cloned() else {
        return response;
    };
    let Ok(limit) = usize::try_from(max_bytes) else {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "pcap buffer limit is too large for this platform",
        );
    };
    let (mut parts, body) = response.into_parts();
    match axum::body::to_bytes(body, limit).await {
        Ok(bytes) => {
            parts.extensions.remove::<PostCompletion>();
            if completion.truncated() {
                parts.headers.insert(
                    HeaderName::from_static("x-evebox-pcap-truncated"),
                    HeaderValue::from_static("true"),
                );
            }
            Response::from_parts(parts, Body::from(bytes))
        }
        Err(_) => error(StatusCode::BAD_GATEWAY, "io", "pcap extraction failed"),
    }
}

/// The empty-but-truncated 200: a size or time limit stopped the
/// extraction before the first matching packet could be written.
/// Truncation is signaled whenever the outcome is known before the
/// headers commit (this empty case, single-chunk outputs whose terminal
/// frame arrived with the data, and buffered POST responses). A native
/// GET that truncates after streaming starts cannot revise its headers.
fn empty_truncated_response(audit: &AuditContext, filename: &str) -> Response {
    let mut headers = pcap_headers(audit, filename);
    headers.insert(
        HeaderName::from_static("x-evebox-pcap-truncated"),
        HeaderValue::from_static("true"),
    );
    (headers, Body::empty()).into_response()
}

#[cfg(not(windows))]
#[allow(clippy::too_many_arguments)]
async fn stream_local(
    context: Arc<ServerContext>,
    source: PcapSource,
    request: PcapRequest,
    filename: String,
    audit: AuditContext,
    global_permit: tokio::sync::OwnedSemaphorePermit,
    source_permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<Response, Response> {
    let settings = &context.pcap.settings;
    // `request_timeout` is the first-frame deadline; once streaming,
    // a client that stops reading is bounded by the stall timeout.
    let request_timeout = settings.request_timeout;
    let stall_timeout = settings.stall_timeout;
    let grace = settings.wedge_grace;

    let (tx, mut rx) = mpsc::channel::<Frame>(16);
    let cancel = CancellationToken::new();
    let reason = CancelReason::new();
    let audit = Arc::new(audit);

    // The progress clock shared by the writer and the supervisor: a
    // single `Instant` taken before the producer spawns, and a
    // millis-since-`started` stamp updated on every frame the writer
    // actually hands off. Initialized to 0 (== progress at supervisor
    // start), so the supervisor's idle watchdog measures output flow,
    // not wall-clock, and never reaps a slow-but-live client whose
    // frames keep advancing this stamp.
    let started = std::time::Instant::now();
    let last_progress = Arc::new(AtomicU64::new(0));

    // The producer: the blocking fetch, framing its output into the
    // channel and always trying to end with a terminal frame.
    let fetch_cancel = cancel.clone();
    let inflight = context.pcap.inflight.clone();
    let writer_progress = last_progress.clone();
    let mut handle = tokio::task::spawn_blocking(move || {
        let _inflight = InflightGuard::arm(inflight);
        let mut writer = ChannelWriter {
            tx,
            buf: Vec::with_capacity(CHUNK_SIZE),
            cancel: fetch_cancel.clone(),
            stall_timeout,
            started,
            last_progress: writer_progress,
        };
        let result = run_extraction(&source, &request, &mut writer, &fetch_cancel);
        finish_producer(result, &mut writer)
    });

    // The supervisor owns the producer from spawn: the join handle,
    // the global permit, and the request's outcome logging. On cancellation the
    // producer gets a bounded grace to acknowledge, so a fetch wedged
    // in a single blocking read cannot pin the permits until restart.
    {
        let audit = audit.clone();
        let cancel = cancel.clone();
        let reason = reason.clone();
        let last_progress = last_progress.clone();
        // The idle watchdog's ceiling: a producer that hands off NO
        // frame for this long has stalled its output (a fetch wedged
        // in a single non-returning read after the first chunk, while
        // the client stays connected and idle). It is deliberately
        // longer than the engine deadline (== `request_timeout`), so a
        // healthy long non-matching scan is reaped by the engine
        // between packets, not here; and longer than one writer
        // `stall_timeout`, so a slow-but-live client — whose parked
        // sends complete within `stall_timeout`, each advancing
        // `last_progress` — is never mistaken for a wedge. A client
        // that stalls without draining is already reaped by the
        // writer's own `stall_timeout`, so this ceiling only ever
        // fires for a genuinely stalled producer.
        let idle_bound = request_timeout + stall_timeout + grace;
        // Poll cadence for the idle check: fine enough to catch a
        // wedge near `idle_bound`, bounded to [100ms, 1s] so it is
        // negligible overhead at the 60s production defaults yet
        // responsive under the tiny timeouts the tests use.
        let check_interval = stall_timeout
            .min(std::time::Duration::from_secs(1))
            .max(std::time::Duration::from_millis(100));
        tokio::spawn(async move {
            let outcome = await_outcome(
                &mut handle,
                &cancel,
                started,
                &last_progress,
                idle_bound,
                check_interval,
                grace,
            )
            .await;
            match outcome {
                None => {
                    // A wedged producer: either it was cancelled and
                    // the grace expired without it acknowledging, or
                    // the idle watchdog saw no output frame for the
                    // whole `idle_bound` while it stayed unresolved.
                    // Both mean the blocking thread is stuck; release
                    // its slots and leave it detached.
                    warn!(
                        "pcap: user={:?} remote={:?} event={:?} outcome=wedged",
                        audit.user, audit.remote, audit.event_id
                    );
                    warn!(
                        "Pcap extraction for event {:?} did not stop (no output progress for {:?}); its thread is still running detached, releasing its slots anyway",
                        audit.event_id, idle_bound
                    );
                }
                Some(Err(err)) => {
                    warn!(
                        "pcap: user={:?} remote={:?} event={:?} outcome=join-error error={}",
                        audit.user, audit.remote, audit.event_id, err
                    );
                }
                Some(Ok(Ok(stats))) => match reason.get() {
                    CancelCause::Timeout => {
                        warn!(
                            "pcap: user={:?} remote={:?} event={:?} outcome=timeout",
                            audit.user, audit.remote, audit.event_id
                        );
                    }
                    CancelCause::Client => {
                        warn!(
                            "pcap: user={:?} remote={:?} event={:?} outcome=aborted reason=client-closed bytes={}",
                            audit.user, audit.remote, audit.event_id, stats.bytes
                        );
                    }
                    // Defensive: a cancelled token always has its
                    // cause set first, but never log a cancelled
                    // fetch as a success.
                    CancelCause::None if cancel.is_cancelled() => {
                        warn!(
                            "pcap: user={:?} remote={:?} event={:?} outcome=aborted reason=client-closed bytes={}",
                            audit.user, audit.remote, audit.event_id, stats.bytes
                        );
                    }
                    CancelCause::None => {
                        log_success(&audit, &stats);
                    }
                },
                Some(Ok(Err(err))) => {
                    if reason.get() == CancelCause::Timeout {
                        warn!(
                            "pcap: user={:?} remote={:?} event={:?} outcome=timeout",
                            audit.user, audit.remote, audit.event_id
                        );
                    } else {
                        match &err {
                            FetchError::Io(io) if io.kind() == std::io::ErrorKind::TimedOut => {
                                // The ChannelWriter stall timeout: the
                                // client kept the connection open but
                                // stopped reading.
                                warn!(
                                    "pcap: user={:?} remote={:?} event={:?} outcome=aborted reason=client-stalled",
                                    audit.user, audit.remote, audit.event_id
                                );
                            }
                            FetchError::Io(io) if io.kind() == std::io::ErrorKind::BrokenPipe => {
                                // The channel receiver vanished
                                // mid-write: the response body was
                                // dropped or cancelled.
                                warn!(
                                    "pcap: user={:?} remote={:?} event={:?} outcome=aborted reason=client-closed",
                                    audit.user, audit.remote, audit.event_id
                                );
                            }
                            FetchError::NoCandidateFiles => {
                                warn!(
                                    "pcap: user={:?} remote={:?} event={:?} outcome=no-candidate-files",
                                    audit.user, audit.remote, audit.event_id
                                );
                            }
                            FetchError::NoMatch(stats) => {
                                warn!(
                                    "pcap: user={:?} remote={:?} event={:?} outcome=no-match files_scanned={} files_vanished={}",
                                    audit.user,
                                    audit.remote,
                                    audit.event_id,
                                    stats.files_scanned,
                                    stats.files_vanished,
                                );
                            }
                            FetchError::Format(message) => {
                                warn!(
                                    "pcap: user={:?} remote={:?} event={:?} outcome=format error={:?}",
                                    audit.user, audit.remote, audit.event_id, message
                                );
                            }
                            FetchError::Io(io) => {
                                error!(
                                    "pcap: user={:?} remote={:?} event={:?} outcome=io error={}",
                                    audit.user, audit.remote, audit.event_id, io
                                );
                            }
                        }
                    }
                }
            }
            // The global permit releases when the extraction ends (or
            // is given up on), whether or not the browser ever drains
            // the buffered stream tail.
            drop(global_permit);
            drop(source_permit);
        });
    }

    // From here until the body takes over, dropping this future is a
    // client abort: the guard cancels the fetch so it stops within
    // one packet read and the supervisor logs outcome=aborted.
    let mut prestream = DisconnectGuard {
        reason: reason.clone(),
        cancel: cancel.clone(),
        done: false,
    };

    // Wait for the first data, the terminal frame, or the deadline.
    let first = match tokio::time::timeout(request_timeout, rx.recv()).await {
        Err(_) => {
            // First-frame deadline: cause first, then cancel. The
            // supervisor logs the outcome and releases the permits.
            reason.set(CancelCause::Timeout);
            cancel.cancel();
            prestream.disarm();
            return Err(error(
                StatusCode::GATEWAY_TIMEOUT,
                "timeout",
                "pcap extraction did not produce output in time",
            ));
        }
        Ok(None) => {
            // Producer died without a terminal frame: it panicked.
            // The supervisor logs the join error.
            prestream.disarm();
            return Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "pcap extraction failed",
            ));
        }
        Ok(Some(Frame::Done(end))) => {
            // Ended before any output byte: pure response shaping;
            // the supervisor logs the outcome.
            prestream.disarm();
            return Err(empty_response(end, &audit, &filename));
        }
        Ok(Some(Frame::Data(first))) => first,
    };

    let mut queued = VecDeque::new();
    queued.push_back(first);

    // The producer may already be done: when the whole output fit one
    // chunk the terminal frame is right behind it and the outcome —
    // including limit truncation — is knowable BEFORE the headers
    // commit. Best effort by design: a Done racing in a microsecond
    // later just streams normally without the marker.
    match rx.try_recv() {
        Ok(Frame::Done(end)) => {
            prestream.disarm();
            return match end {
                ProducerEnd::Format(message) => {
                    // An honest error beats a 200 with a broken tail:
                    // discard the data, no headers were sent yet.
                    Err(error(StatusCode::BAD_GATEWAY, "format", &message))
                }
                ProducerEnd::Io => Err(error(
                    StatusCode::BAD_GATEWAY,
                    "io",
                    "pcap extraction failed",
                )),
                end => {
                    // Complete. NoMatch/NoCandidateFiles cannot follow
                    // data; treat them defensively as an untruncated
                    // end.
                    let truncated = matches!(
                        end,
                        ProducerEnd::Complete {
                            truncated: true,
                            ..
                        }
                    );
                    let mut headers = pcap_headers(&audit, &filename);
                    if truncated {
                        headers.insert(
                            HeaderName::from_static("x-evebox-pcap-truncated"),
                            HeaderValue::from_static("true"),
                        );
                    }
                    let chunk = queued.pop_front().unwrap_or_default();
                    // The producer is done: a clean single-chunk body,
                    // no guard needed.
                    Ok((headers, Body::from(chunk)).into_response())
                }
            };
        }
        Ok(Frame::Data(chunk)) => queued.push_back(chunk),
        Err(_) => {}
    }

    // Streaming: the disconnect guard rides in the body stream state
    // from here — dropping the body mid-stream is the client abort.
    let headers = pcap_headers(&audit, &filename);
    let completion = (!audit.native).then(PostCompletion::default);
    let guard = DisconnectGuard {
        reason,
        cancel,
        done: false,
    };
    prestream.disarm();
    let state = BodyState {
        rx,
        guard,
        queued,
        finished: false,
        completion: completion.clone(),
    };
    let mut response = (headers, Body::from_stream(body_stream(state))).into_response();
    if let Some(completion) = completion {
        response.extensions_mut().insert(completion);
    }
    Ok(response)
}

/// Removes the server-side task when the request ends, and best-effort
/// cancels the agent job unless a real terminal result already arrived. A
/// cancel that cannot be delivered is fine: a job whose connection is gone is
/// cancelled by the agent itself.
struct RemoteTaskGuard {
    tasks: Arc<tasks::Registry>,
    entry: Arc<AgentEntry>,
    id: String,
    token: String,
    cancel_on_drop: bool,
}

impl RemoteTaskGuard {
    fn disarm_cancel(&mut self) {
        self.cancel_on_drop = false;
    }
}

impl Drop for RemoteTaskGuard {
    fn drop(&mut self) {
        if self.cancel_on_drop {
            let _ = self.entry.try_send(ServerMessage::Cancel {
                id: self.id.clone(),
                token: self.token.clone(),
            });
        }
        self.tasks.remove(&self.id, &self.token);
    }
}

enum RemoteFirst {
    Data(Bytes),
    /// A terminal outcome (never `Wait`) plus the result's statistics for
    /// the audit log.
    Terminal(RemoteOutcome, Option<crate::agent::protocol::WireStats>),
    UploadFailed(&'static str),
    Timeout,
}

#[allow(clippy::too_many_arguments)]
async fn stream_agent(
    context: Arc<ServerContext>,
    entry: Arc<AgentEntry>,
    request: PcapRequest,
    filename: String,
    audit: AuditContext,
    global_permit: tokio::sync::OwnedSemaphorePermit,
    source_permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<Response, Response> {
    let request_timeout = context.pcap.settings.request_timeout;
    let stall_timeout = context.pcap.settings.stall_timeout;
    let id = uuid::Uuid::new_v4().to_string();
    let token = generate_job_token();
    let start_us = request.start.ok_or_else(|| {
        fail(
            &audit,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "normalized pcap request has no start time",
        )
    })?;
    let end_us = request.end.ok_or_else(|| {
        fail(
            &audit,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "normalized pcap request has no end time",
        )
    })?;
    let limits = WireLimits::try_from(&request.limits).map_err(|message| {
        fail(
            &audit,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            message,
        )
    })?;
    let message = ServerMessage::PcapRequest {
        id: id.clone(),
        token: token.clone(),
        filter: WirePcapFilter::from(request.filter.as_ref()),
        start_us,
        end_us,
        limits,
    };
    let control_bytes = serde_json::to_vec(&message).map_err(|_| {
        fail(
            &audit,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "could not encode remote pcap request",
        )
    })?;
    if control_bytes.len() > CONTROL_MESSAGE_MAX_BYTES {
        return Err(fail(
            &audit,
            StatusCode::PAYLOAD_TOO_LARGE,
            "control-message-too-large",
            "pcap filter is too large for the agent control channel",
        ));
    }

    // Register before dispatch so a fast upload cannot race the job map.
    let tasks::Handles {
        mut body_rx,
        mut upload_rx,
        mut result_rx,
    } = context
        .pcap_tasks
        .register(
            id.clone(),
            token.clone(),
            entry.name.clone(),
            entry.generation,
            request.limits.max_bytes,
        )
        .map_err(|_| {
            fail(
                &audit,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "duplicate remote pcap job id",
            )
        })?;
    let mut task_guard = RemoteTaskGuard {
        tasks: context.pcap_tasks.clone(),
        entry: entry.clone(),
        id: id.clone(),
        token: token.clone(),
        cancel_on_drop: true,
    };

    if context.agents.try_send_current(&entry, message).is_err() {
        // The request never reached the agent; there is nothing to cancel.
        task_guard.disarm_cancel();
        return Err(fail(
            &audit,
            StatusCode::SERVICE_UNAVAILABLE,
            "agent-unavailable",
            "pcap agent is not accepting requests",
        ));
    }

    let cancel = CancellationToken::new();
    let reason = CancelReason::new();
    let mut prestream = DisconnectGuard {
        reason: reason.clone(),
        cancel: cancel.clone(),
        done: false,
    };
    let mut terminal: Option<PcapResult> = None;
    let mut body_open = true;
    let mut result_open = true;

    let first = tokio::time::timeout(request_timeout, async {
        loop {
            // Result delivery and upload start are serialized by the job's
            // body-sender lock. Refresh the watch snapshot before evaluating
            // a terminal result so a ready result cannot win `select!` and be
            // validated against the stale Pending value from the prior turn.
            let upload = upload_rx.borrow().clone();
            if let UploadState::Failed { reason, .. } = upload {
                break RemoteFirst::UploadFailed(reason);
            }
            if let Some(result) = terminal.as_ref() {
                match tasks::classify_result(result, &upload, body_open, 0) {
                    RemoteOutcome::Wait => {}
                    outcome => break RemoteFirst::Terminal(outcome, result.stats),
                }
            }

            tokio::select! {
                chunk = body_rx.recv(), if body_open => match chunk {
                    Some(chunk) => break RemoteFirst::Data(chunk),
                    None => body_open = false,
                },
                result = &mut result_rx, if result_open => {
                    result_open = false;
                    terminal = Some(result.unwrap_or(PcapResult {
                        code: PcapResultCode::Error,
                        upload: PcapUploadStatus::Failed,
                        message: Some("agent result channel closed".to_string()),
                        stats: None,
                    }));
                },
                changed = upload_rx.changed() => {
                    if changed.is_err() {
                        break RemoteFirst::UploadFailed("upload-state-closed");
                    }
                },
            }
        }
    })
    .await
    .unwrap_or(RemoteFirst::Timeout);

    match first {
        RemoteFirst::Timeout => {
            reason.set(CancelCause::Timeout);
            cancel.cancel();
            prestream.disarm();
            log_remote_failure(
                &audit,
                "timeout",
                "pcap agent did not produce output in time",
                None,
            );
            Err(error(
                StatusCode::GATEWAY_TIMEOUT,
                "timeout",
                "pcap agent did not produce output in time",
            ))
        }
        RemoteFirst::UploadFailed(upload_reason) => {
            prestream.disarm();
            log_remote_failure(&audit, "agent-upload", upload_reason, None);
            Err(error(
                StatusCode::BAD_GATEWAY,
                "agent-upload",
                upload_reason,
            ))
        }
        RemoteFirst::Terminal(outcome, stats) => {
            // A real terminal result means the agent's job is finished;
            // there is nothing left to cancel.
            task_guard.disarm_cancel();
            prestream.disarm();
            Err(remote_empty_response(outcome, stats, &audit, &filename))
        }
        RemoteFirst::Data(first) => {
            let audit = Arc::new(audit);
            let headers = pcap_headers(&audit, &filename);
            let completion = (!audit.native).then(PostCompletion::default);
            let (frame_tx, frame_rx) = mpsc::channel::<Frame>(2);
            let supervisor_cancel = cancel.clone();
            let supervisor_audit = audit.clone();
            let first_len = u64::try_from(first.len()).unwrap_or(u64::MAX);
            let mut upload = upload_rx.borrow().clone();

            tokio::spawn(async move {
                let mut task_guard = task_guard;
                // Both concurrency gates stay closed until the remote
                // producer is finished, not merely until the response ends.
                let _global_permit = global_permit;
                let _source_permit = source_permit;
                let mut bytes = first_len;
                let mut result = terminal;
                let mut success_stats: Option<FetchStats> = None;
                let mut failure = (
                    "agent-error",
                    "remote pcap stream ended unexpectedly".to_string(),
                );

                let producer_end = loop {
                    if let UploadState::Failed { reason, .. } = &upload {
                        failure = ("agent-upload-error", (*reason).to_string());
                        break ProducerEnd::Io;
                    }

                    if let Some(result) = result.as_ref() {
                        match tasks::classify_result(result, &upload, body_open, bytes) {
                            RemoteOutcome::Wait => {}
                            RemoteOutcome::Complete { stats } => {
                                success_stats = Some(stats.into());
                                break ProducerEnd::Complete {
                                    truncated: stats.truncated,
                                };
                            }
                            // Unreachable with streamed bytes; the classifier
                            // reports empty results after data as Protocol.
                            RemoteOutcome::NoCandidateFiles | RemoteOutcome::NoMatch => {
                                failure = (
                                    "agent-protocol",
                                    "empty result arrived after an upload had started".to_string(),
                                );
                                break ProducerEnd::Io;
                            }
                            RemoteOutcome::Cancelled { message } => {
                                failure = (
                                    "agent-cancelled",
                                    message.unwrap_or_else(|| {
                                        "pcap agent cancelled the request".to_string()
                                    }),
                                );
                                break ProducerEnd::Io;
                            }
                            RemoteOutcome::Error { message } => {
                                failure = ("agent-error", message);
                                break ProducerEnd::Io;
                            }
                            RemoteOutcome::Protocol { detail } => {
                                failure = ("agent-protocol", detail);
                                break ProducerEnd::Io;
                            }
                        }
                    }

                    enum Event {
                        Data(Option<Bytes>),
                        Upload(bool),
                        Result(Option<PcapResult>),
                        Cancelled,
                        Idle,
                    }
                    let event = tokio::select! {
                        _ = supervisor_cancel.cancelled() => Event::Cancelled,
                        chunk = body_rx.recv(), if body_open => Event::Data(chunk),
                        changed = upload_rx.changed() => Event::Upload(changed.is_ok()),
                        received = &mut result_rx, if result_open => {
                            result_open = false;
                            Event::Result(received.ok())
                        }
                        _ = tokio::time::sleep(stall_timeout) => Event::Idle,
                    };

                    match event {
                        Event::Data(Some(chunk)) => {
                            let len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
                            let Some(total) = bytes.checked_add(len) else {
                                failure = (
                                    "agent-protocol",
                                    "pcap upload byte count overflowed".to_string(),
                                );
                                break ProducerEnd::Io;
                            };
                            if total > request.limits.max_bytes {
                                failure = (
                                    "agent-protocol",
                                    "pcap upload exceeded its dispatched byte limit".to_string(),
                                );
                                break ProducerEnd::Io;
                            }
                            match tokio::time::timeout(
                                stall_timeout,
                                frame_tx.send(Frame::Data(chunk)),
                            )
                            .await
                            {
                                Ok(Ok(())) => bytes = total,
                                Ok(Err(_)) => {
                                    failure = (
                                        "client-closed",
                                        "browser response body closed".to_string(),
                                    );
                                    break ProducerEnd::Io;
                                }
                                Err(_) => {
                                    failure = (
                                        "client-stalled",
                                        "browser did not drain pcap output".to_string(),
                                    );
                                    break ProducerEnd::Io;
                                }
                            }
                        }
                        Event::Data(None) => body_open = false,
                        Event::Upload(true) => upload = upload_rx.borrow().clone(),
                        Event::Upload(false) => {
                            upload = UploadState::Failed {
                                reason: "upload-state-closed",
                                bytes,
                            };
                        }
                        Event::Result(Some(received)) => result = Some(received),
                        Event::Result(None) => {
                            failure = ("agent-error", "agent result channel closed".to_string());
                            break ProducerEnd::Io;
                        }
                        Event::Cancelled => {
                            failure = (
                                "client-closed",
                                "browser cancelled the pcap response".to_string(),
                            );
                            break ProducerEnd::Io;
                        }
                        Event::Idle => {
                            failure = if !body_open
                                && result.is_none()
                                && matches!(upload, UploadState::Complete { .. })
                            {
                                (
                                    "agent-result-timeout",
                                    "pcap agent omitted its terminal result after upload EOF"
                                        .to_string(),
                                )
                            } else {
                                (
                                    "agent-stalled",
                                    "pcap agent made no upload or result progress".to_string(),
                                )
                            };
                            break ProducerEnd::Io;
                        }
                    }
                };

                let clean = matches!(producer_end, ProducerEnd::Complete { .. });
                if clean {
                    // A verified terminal result means the agent's job is
                    // finished; there is nothing left to cancel.
                    task_guard.disarm_cancel();
                }
                // Release the task entry and per-source lane before the
                // browser drains any buffered body tail.
                drop(task_guard);
                let _ =
                    tokio::time::timeout(stall_timeout, frame_tx.send(Frame::Done(producer_end)))
                        .await;
                if clean {
                    // Like the local path, success is the producer completing
                    // cleanly; delivery of the buffered tail is the browser's
                    // problem and the disconnect guard's to observe.
                    if let Some(stats) = success_stats.as_ref() {
                        log_success(&supervisor_audit, stats);
                    }
                    return;
                }
                log_remote_failure(
                    &supervisor_audit,
                    failure.0,
                    &failure.1,
                    result.and_then(|result| result.stats),
                );
            });

            let guard = DisconnectGuard {
                reason,
                cancel,
                done: false,
            };
            let mut queued = VecDeque::new();
            queued.push_back(first);
            let state = BodyState {
                rx: frame_rx,
                guard,
                queued,
                finished: false,
                completion: completion.clone(),
            };
            prestream.disarm();
            let mut response = (headers, Body::from_stream(body_stream(state))).into_response();
            if let Some(completion) = completion {
                response.extensions_mut().insert(completion);
            }
            Ok(response)
        }
    }
}

fn generate_job_token() -> String {
    use rand::RngCore;

    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

fn log_remote_failure(
    audit: &AuditContext,
    outcome: &str,
    message: &str,
    stats: Option<crate::agent::protocol::WireStats>,
) {
    let stats = stats.unwrap_or_default();
    warn!(
        "pcap: user={:?} remote={:?} event={:?} source={:?} mode={} filter={:?} window={:?} outcome={} message={:?} packets={} bytes={} files_scanned={} files_vanished={} truncated={}",
        audit.user,
        audit.remote,
        audit.event_id,
        audit.source,
        audit.mode,
        audit.filter,
        audit.window,
        outcome,
        message,
        stats.packets,
        stats.bytes,
        stats.files_scanned,
        stats.files_vanished,
        stats.truncated,
    );
}

/// Map a classified terminal outcome to the empty (no body bytes streamed)
/// HTTP response, with once-only accounting and one audit line.
fn remote_empty_response(
    outcome: RemoteOutcome,
    result_stats: Option<crate::agent::protocol::WireStats>,
    audit: &AuditContext,
    filename: &str,
) -> Response {
    match outcome {
        RemoteOutcome::Wait => unreachable!("Wait is not a terminal outcome"),
        RemoteOutcome::Complete { stats } => {
            let truncated = stats.truncated;
            let stats: FetchStats = stats.into();
            log_success(audit, &stats);
            empty_response(ProducerEnd::Complete { truncated }, audit, filename)
        }
        RemoteOutcome::NoCandidateFiles => {
            log_remote_failure(
                audit,
                "no-candidate-files",
                "no pcap files cover the requested time window",
                result_stats,
            );
            empty_response(ProducerEnd::NoCandidateFiles, audit, filename)
        }
        RemoteOutcome::NoMatch => {
            log_remote_failure(
                audit,
                "no-match",
                "no packets matched the requested filter",
                result_stats,
            );
            empty_response(ProducerEnd::NoMatch, audit, filename)
        }
        RemoteOutcome::Cancelled { message } => {
            let message = message.unwrap_or_else(|| "pcap agent cancelled the request".to_string());
            log_remote_failure(audit, "agent-cancelled", &message, result_stats);
            error(StatusCode::BAD_GATEWAY, "agent-cancelled", &message)
        }
        RemoteOutcome::Error { message } => {
            log_remote_failure(audit, "agent-error", &message, result_stats);
            error(StatusCode::BAD_GATEWAY, "agent-error", &message)
        }
        RemoteOutcome::Protocol { detail } => {
            log_remote_failure(audit, "agent-protocol", &detail, result_stats);
            error(
                StatusCode::BAD_GATEWAY,
                "agent-protocol",
                "pcap agent returned an inconsistent terminal result",
            )
        }
    }
}

/// The joined producer result the supervisor observes: the inner
/// `Result` is the fetch outcome, the outer is the join status.
#[cfg(not(windows))]
type JoinedFetch = Result<Result<FetchStats, FetchError>, tokio::task::JoinError>;

/// Wait for the producer's outcome under three bounds, returning
/// `Some(joined)` when it resolves and `None` when it must be
/// abandoned as wedged (permits released, thread left detached):
/// - it resolves on its own → `Some(join result)` (the happy path and
///   every normal error take this immediately);
/// - the token is cancelled (client disconnect / first-byte timeout) →
///   a bounded `grace` for it to acknowledge, then `Some` or `None`;
/// - it is neither resolving nor cancelled but has handed off NO output
///   frame for `idle_bound` (a fetch wedged inside a single
///   non-returning read after the first chunk, client connected and
///   idle) → `None`. Progress is measured as output flow: `started`
///   plus the `last_progress` stamp the writer advances on every frame,
///   so a slow-but-live client (whose parked sends keep completing and
///   stamping progress) is never reaped here.
#[cfg(not(windows))]
async fn await_outcome(
    handle: &mut tokio::task::JoinHandle<Result<FetchStats, FetchError>>,
    cancel: &CancellationToken,
    started: std::time::Instant,
    last_progress: &AtomicU64,
    idle_bound: std::time::Duration,
    check_interval: std::time::Duration,
    grace: std::time::Duration,
) -> Option<JoinedFetch> {
    let idle_bound_ms = idle_bound.as_millis() as u64;
    loop {
        tokio::select! {
            result = &mut *handle => break Some(result),
            _ = cancel.cancelled() => {
                break tokio::time::timeout(grace, &mut *handle).await.ok();
            }
            _ = tokio::time::sleep(check_interval) => {
                let elapsed = started.elapsed().as_millis() as u64;
                let idle = elapsed.saturating_sub(last_progress.load(Ordering::Relaxed));
                if idle >= idle_bound_ms {
                    // No output progress for the whole ceiling: wedged.
                    break None;
                }
                // Otherwise keep waiting: frames are still flowing (or
                // the fetch is genuinely still scanning within bound).
            }
        }
    }
}

/// Ship the producer's terminal frame and settle the returned result.
///
/// A client-caused fetch error (the writer already saw the receiver
/// gone or a stall) skips the Done send — nobody is reading, and
/// another bounded send would only delay the permits by one more stall
/// timeout; the channel-closed-without-Done path carries the semantics
/// whenever a Done goes missing. Otherwise the Done is sent, and if the
/// fetch SUCCEEDED but that Done could not be delivered (client stalled
/// at the very tail with the channel full, or the receiver vanished),
/// the success is DOWNGRADED to the send's io error: the client's body
/// ends without a Done and hyper tears it, so the supervisor reports
/// the torn transfer as client-stalled / client-closed, not outcome=ok.
#[cfg(not(windows))]
fn finish_producer(
    result: Result<FetchStats, FetchError>,
    writer: &mut ChannelWriter,
) -> Result<FetchStats, FetchError> {
    let client_gone = matches!(
        &result,
        Err(FetchError::Io(err)) if matches!(
            err.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::BrokenPipe
        )
    );
    if client_gone {
        return result;
    }
    if let Err(err) = writer.send_frame(Frame::Done(producer_end(&result)))
        && result.is_ok()
        && matches!(
            err.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::BrokenPipe
        )
    {
        return Err(FetchError::Io(err));
    }
    result
}

/// The blocking producer body: run the fetch, then push the buffered
/// tail with a final flush whose error propagates — a client that
/// stalls at the very tail must not be reported as outcome=ok.
#[cfg(not(windows))]
fn run_extraction(
    source: &PcapSource,
    request: &PcapRequest,
    writer: &mut ChannelWriter,
    cancel: &CancellationToken,
) -> Result<FetchStats, FetchError> {
    let result = pcap::fetch(source, request, writer, cancel);
    result.and_then(|stats| writer.flush().map(|()| stats).map_err(FetchError::Io))
}

/// A message from the extraction producer to the response side: a
/// chunk of pcap output, or the terminal frame saying how the
/// producer ended.
enum Frame {
    Data(Bytes),
    Done(ProducerEnd),
}

/// How the producer ended, for response shaping only. The supervisor
/// logs details from the richer `Result<FetchStats, FetchError>` it
/// joins.
enum ProducerEnd {
    Complete {
        truncated: bool,
    },
    NoCandidateFiles,
    NoMatch,
    // Only the local producer constructs `Format`; the shared response
    // side still matches it on Windows.
    #[cfg_attr(windows, allow(dead_code))]
    Format(String),
    /// The detail is logged by the supervisor; the response body
    /// stays generic.
    Io,
}

/// Derive the terminal frame from the producer's result.
#[cfg(not(windows))]
fn producer_end(result: &Result<FetchStats, FetchError>) -> ProducerEnd {
    match result {
        Ok(stats) => ProducerEnd::Complete {
            truncated: stats.truncated,
        },
        Err(FetchError::NoCandidateFiles) => ProducerEnd::NoCandidateFiles,
        Err(FetchError::NoMatch(_)) => ProducerEnd::NoMatch,
        Err(FetchError::Format(message)) => ProducerEnd::Format(message.clone()),
        Err(FetchError::Io(_)) => ProducerEnd::Io,
    }
}

/// Why the fetch was cancelled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CancelCause {
    None = 0,
    Timeout = 1,
    Client = 2,
}

/// Shared cancellation-cause cell: `set` is a compare-exchange from
/// None, so the FIRST cause wins. Always set the cause BEFORE
/// cancelling the token, so the supervisor never observes a
/// cancelled token without its cause.
#[derive(Clone)]
struct CancelReason(Arc<AtomicU8>);

impl CancelReason {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(CancelCause::None as u8)))
    }

    fn set(&self, cause: CancelCause) {
        let _ = self.0.compare_exchange(
            CancelCause::None as u8,
            cause as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    #[cfg(not(windows))]
    fn get(&self) -> CancelCause {
        match self.0.load(Ordering::SeqCst) {
            value if value == CancelCause::Timeout as u8 => CancelCause::Timeout,
            value if value == CancelCause::Client as u8 => CancelCause::Client,
            _ => CancelCause::None,
        }
    }
}

/// Counts the blocking extraction closure in the service's
/// in-flight gauge for the closure's whole lifetime; the Drop makes
/// it panic-safe.
#[cfg(not(windows))]
struct InflightGuard(Arc<AtomicUsize>);

#[cfg(not(windows))]
impl InflightGuard {
    fn arm(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

#[cfg(not(windows))]
impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// State for the streamed response body: everything whose Drop
/// matters on a mid-download disconnect rides here, so dropping the
/// body drops the guard (cancel) and the receiver.
struct BodyState {
    rx: mpsc::Receiver<Frame>,
    guard: DisconnectGuard,
    /// Chunks the handler already took off the channel.
    queued: VecDeque<Bytes>,
    finished: bool,
    /// Present only for the buffered POST path, whose wrapper waits for the
    /// body to observe Done before committing response headers.
    completion: Option<PostCompletion>,
}

/// Terminal status shared between the streaming body and the POST wrapper.
#[derive(Clone, Default)]
struct PostCompletion(Arc<AtomicBool>);

impl PostCompletion {
    fn mark_truncated(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn truncated(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// The streamed body with honest termination: a producer failure —
/// or a producer that vanished without its terminal frame — surfaces
/// as a stream error, so hyper aborts the connection instead of
/// writing a clean chunked EOF over a truncated pcap. The clean end
/// is reserved for producer success, including deliberate
/// limit-truncation where delivering the partial capture is the
/// point.
fn body_stream(state: BodyState) -> impl futures::Stream<Item = Result<Bytes, std::io::Error>> {
    futures::stream::unfold(state, |mut state| async move {
        if let Some(chunk) = state.queued.pop_front() {
            return Some((Ok(chunk), state));
        }
        if state.finished {
            return None;
        }
        match state.rx.recv().await {
            Some(Frame::Data(chunk)) => Some((Ok(chunk), state)),
            Some(Frame::Done(end)) => {
                if matches!(
                    &end,
                    ProducerEnd::Complete {
                        truncated: true,
                        ..
                    }
                ) && let Some(completion) = &state.completion
                {
                    completion.mark_truncated();
                }
                state.guard.disarm();
                state.finished = true;
                match end {
                    ProducerEnd::Format(_) | ProducerEnd::Io => {
                        Some((Err(std::io::Error::other("pcap extraction failed")), state))
                    }
                    _ => None,
                }
            }
            None => {
                // Closed without a terminal frame: a producer panic,
                // or a Done dropped on a stalled client. The producer
                // is gone either way; disarm and end dirty.
                state.guard.disarm();
                state.finished = true;
                Some((
                    Err(std::io::Error::other("pcap extraction ended unexpectedly")),
                    state,
                ))
            }
        }
    })
}

/// Cancels the fetch as client-caused when dropped while still
/// armed. One instance covers the pre-stream window (the request
/// future dropped before the body was built — a client abort before
/// the first byte must still cancel the fetch), another rides in the
/// body stream state (browser disconnect mid-download). Disarmed on
/// normal completion so a fully-delivered response never reads as a
/// disconnect.
struct DisconnectGuard {
    reason: CancelReason,
    cancel: CancellationToken,
    done: bool,
}

impl DisconnectGuard {
    /// Normal completion or handoff: do not cancel on drop.
    fn disarm(&mut self) {
        self.done = true;
    }
}

impl Drop for DisconnectGuard {
    fn drop(&mut self) {
        if !self.done {
            self.reason.set(CancelCause::Client);
            self.cancel.cancel();
        }
    }
}

/// A minimal but valid classic pcap file: the 24-byte global header
/// with zero packet records. Little-endian, microsecond magic, Ethernet
/// linktype — the linktype is cosmetic with no packets to interpret, so
/// any capture tool opens this and reports zero packets.
fn empty_pcap_bytes() -> [u8; 24] {
    let mut header = [0u8; 24];
    header[0..4].copy_from_slice(&0xa1b2c3d4u32.to_le_bytes()); // magic
    header[4..6].copy_from_slice(&2u16.to_le_bytes()); // version major
    header[6..8].copy_from_slice(&4u16.to_le_bytes()); // version minor
    // thiszone (i32) and sigfigs (u32) are left zero.
    header[16..20].copy_from_slice(&262144u32.to_le_bytes()); // snaplen
    header[20..24].copy_from_slice(&1u32.to_le_bytes()); // network = LINKTYPE_ETHERNET
    header
}

/// Shape the response for a fetch that ended before any output byte,
/// purely from the terminal frame; outcome logging lives in the
/// supervisor.
fn empty_response(end: ProducerEnd, audit: &AuditContext, filename: &str) -> Response {
    // Empty non-fault outcomes on the native path become a valid empty capture.
    // Genuine format/I/O faults remain structured errors; the webapp's hidden
    // same-origin frame reads them and surfaces an in-app notification.
    if audit.native {
        return match end {
            ProducerEnd::Format(message) => error(StatusCode::BAD_GATEWAY, "format", &message),
            ProducerEnd::Io => error(StatusCode::BAD_GATEWAY, "io", "pcap extraction failed"),
            _ => (
                pcap_headers(audit, filename),
                Body::from(empty_pcap_bytes().to_vec()),
            )
                .into_response(),
        };
    }
    match end {
        // A size or time limit stopped extraction before the first
        // matching packet.
        ProducerEnd::Complete {
            truncated: true, ..
        } => empty_truncated_response(audit, filename),
        // Defensive: a complete fetch with no output and no
        // truncation maps to NoMatch in the engine, but an empty
        // capture is the honest shape if it ever arrives.
        ProducerEnd::Complete {
            truncated: false, ..
        } => (pcap_headers(audit, filename), Body::empty()).into_response(),
        ProducerEnd::NoCandidateFiles => error(
            StatusCode::NOT_FOUND,
            "no-candidate-files",
            "no pcap files cover the requested time window",
        ),
        ProducerEnd::NoMatch => error(
            StatusCode::NOT_FOUND,
            "no-match",
            "no packets matched the event's flow",
        ),
        ProducerEnd::Format(message) => error(StatusCode::BAD_GATEWAY, "format", &message),
        ProducerEnd::Io => error(StatusCode::BAD_GATEWAY, "io", "pcap extraction failed"),
    }
}

fn log_success(audit: &AuditContext, stats: &FetchStats) {
    info!(
        "pcap: user={:?} remote={:?} event={:?} source={:?} mode={} filter={:?} window={:?} outcome=ok packets={} bytes={} files_scanned={} files_vanished={} truncated={}",
        audit.user,
        audit.remote,
        audit.event_id,
        audit.source,
        audit.mode,
        audit.filter,
        audit.window,
        stats.packets,
        stats.bytes,
        stats.files_scanned,
        stats.files_vanished,
        stats.truncated,
    );
}

/// A `std::io::Write` that frames output into channel messages.
///
/// Sends are bounded: extraction stops (BrokenPipe) once the receiver
/// is gone, and — critically — if the consumer stops draining while
/// keeping the connection open, a send that cannot complete within
/// `stall_timeout` fails too (TimedOut). The failed send ends the
/// blocking fetch, and the supervisor task keyed to the fetch then
/// releases the request's semaphore permits — so a stalled client
/// unpins the blocking thread AND the permits (the deadline in
/// `fetch` is only checked between packets, never while parked
/// mid-send).
#[cfg(not(windows))]
struct ChannelWriter {
    tx: mpsc::Sender<Frame>,
    buf: Vec<u8>,
    cancel: CancellationToken,
    stall_timeout: std::time::Duration,
    /// Shared progress clock start, cloned from the single `Instant`
    /// the supervisor holds.
    started: std::time::Instant,
    /// Millis-since-`started` of the last frame actually handed off,
    /// read by the supervisor's idle watchdog. Advanced on every
    /// successful send so a live-but-slow client keeps proving output
    /// progress.
    last_progress: Arc<AtomicU64>,
}

#[cfg(not(windows))]
impl ChannelWriter {
    /// Push the buffered output as a data frame.
    fn push(&mut self) -> std::io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = Bytes::from(std::mem::replace(
            &mut self.buf,
            Vec::with_capacity(CHUNK_SIZE),
        ));
        self.send_frame(Frame::Data(chunk))
    }

    /// Bounded send of any frame: fails BrokenPipe when the receiver
    /// is gone or the request was cancelled, TimedOut when the
    /// consumer stalls past `stall_timeout`.
    fn send_frame(&mut self, frame: Frame) -> std::io::Result<()> {
        let mut frame = frame;
        let deadline = std::time::Instant::now() + self.stall_timeout;
        loop {
            match self.tx.try_send(frame) {
                Ok(()) => {
                    // A frame was handed off: advance the progress
                    // stamp the supervisor's idle watchdog reads.
                    self.last_progress
                        .store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
                    return Ok(());
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "pcap receiver dropped",
                    ));
                }
                Err(mpsc::error::TrySendError::Full(returned)) => {
                    if self.cancel.is_cancelled() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "pcap request cancelled",
                        ));
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "pcap client stalled",
                        ));
                    }
                    frame = returned;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
    }
}

#[cfg(not(windows))]
impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= CHUNK_SIZE {
            self.push()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.push()
    }
}

/// `{"error": {"code": ..., "message": ...}}` with a status.
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    let body = json!({ "error": { "code": code, "message": message } });
    (status, Json(body)).into_response()
}

/// Log the request's audit line with the error code as the outcome,
/// and build the error response.
fn fail(audit: &AuditContext, status: StatusCode, code: &str, message: &str) -> Response {
    warn!(
        "pcap: user={:?} remote={:?} event={:?} source={:?} mode={} filter={:?} window={:?} outcome={} message={:?}",
        audit.user,
        audit.remote,
        audit.event_id,
        audit.source,
        audit.mode,
        audit.filter,
        audit.window,
        code,
        message
    );
    error(status, code, message)
}

/// The present, non-blank value of an optional string field. Blank
/// (empty or whitespace) reads as absent — for `filter` this collapses
/// "empty" and "absent" to the same match-all/derived semantics, and
/// for the time fields a blank string never selects a mode.
fn present(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Parse a `duration`/`before`/`after` span, defaulting to `1m` when
/// blank or absent, using the shared duration parser.
fn parse_span(value: &Option<String>) -> Result<chrono::Duration, String> {
    let raw = present(value).unwrap_or("1m");
    let std = crate::server::pcap::parse_duration_seconds(raw)?;
    chrono::Duration::from_std(std).map_err(|err| format!("duration out of range: {err}"))
}

/// Parse a per-request `max_size`: a size string (`50mb`, `2gb`) or a
/// bare byte count, using the shared human-size parser.
/// `0`/`none`/`unlimited` lifts the cap entirely (`u64::MAX`).
fn parse_max_bytes(raw: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "0" | "none" | "unlimited" => Ok(u64::MAX),
        _ => crate::util::parse_humansize(&trimmed.to_uppercase())
            // A zero cap (`0mb`, `00`) is nonsensical for extraction and
            // would truncate everything; treat any zero the same as the
            // bare `0` above and lift the cap entirely.
            .map(|bytes| if bytes == 0 { u64::MAX } else { bytes as u64 })
            .map_err(|err| err.to_string()),
    }
}

/// A human-readable `start..end` for the audit line.
fn describe_window(window: &crate::pcap::Window) -> String {
    format!("{}..{}", window.start, window.end)
}

/// A pre-fetch request error: an HTTP status, a machine `code`, and a
/// message. The caller renders it through [`fail`] — which logs the
/// audit line — into the standard
/// `{error:{code,message}}` body. Kept small (rather than a full
/// `Response`) so the request-building helpers' `Result`s do not trip
/// `clippy::result_large_err`.
struct RequestError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl RequestError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad-request", message)
    }

    fn bad_event(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad-event", message)
    }
}

/// Build the engine filter and time window from the request, per the
/// mode precedence:
///
/// 1. `start` present → free-form: window `[start, start+duration]`,
///    filter = the raw BPF when non-empty, else all packets.
/// 2. else `before`/`after` present → event-relative (needs an event):
///    window `[event_ts − before, event_ts + after]`, filter = the raw
///    BPF when non-empty, else the event's derived flow.
/// 3. else an event is present → default: derived flow + derived
///    window (unchanged auto behavior).
/// 4. else → 400: a standalone request needs a start time.
///
/// Sets the audit `mode`/`filter`/`window` fields as it goes. A
/// supplied BPF filter is passed through without validation; the
/// compiling side rejects malformed expressions.
fn build_request(
    audit: &mut AuditContext,
    body: &PcapRequestBody,
    source: Option<&serde_json::Value>,
) -> Result<(Option<PcapFilter>, crate::pcap::Window), RequestError> {
    if let Some(start) = present(&body.start) {
        // Free-form: absolute start plus a span. An event, if any, is
        // used only for audit context and the filename, never for the
        // window.
        audit.mode = "freeform";
        let start = crate::datetime::parse(start, None)
            .map_err(|err| RequestError::bad_request(format!("bad start time: {err}")))?;
        let span = parse_span(&body.duration)
            .map_err(|err| RequestError::bad_request(format!("bad duration: {err}")))?;
        let window = pcap::window_from_start(&start, span)
            .map_err(|err| RequestError::bad_request(err.to_string()))?;
        audit.window = describe_window(&window);
        let filter = build_filter_or_all(audit, &body.filter)?;
        Ok((filter, window))
    } else if present(&body.before).is_some() || present(&body.after).is_some() {
        // Event-relative: the event's window shifted by before/after.
        audit.mode = "relative";
        let source =
            source.ok_or_else(|| RequestError::bad_request("before/after requires an event"))?;
        let before = parse_span(&body.before)
            .map_err(|err| RequestError::bad_request(format!("bad before duration: {err}")))?;
        let after = parse_span(&body.after)
            .map_err(|err| RequestError::bad_request(format!("bad after duration: {err}")))?;
        let window = pcap::window_around_event(source, before, after)
            .map_err(|err| RequestError::bad_event(err.to_string()))?;
        audit.window = describe_window(&window);
        // A non-empty filter overrides the derived flow.
        let filter = match present(&body.filter) {
            Some(expression) => {
                audit.filter = expression.to_string();
                Some(PcapFilter::Expression(expression.to_string()))
            }
            None => Some(PcapFilter::Flow(derive_flow(audit, source)?)),
        };
        Ok((filter, window))
    } else if let Some(source) = source {
        // Default (unchanged): derived flow + derived window.
        audit.mode = "default";
        let selector = derive_flow(audit, source)?;
        let window =
            pcap::derive_window(source).map_err(|err| RequestError::bad_event(err.to_string()))?;
        audit.window = describe_window(&window);
        Ok((Some(PcapFilter::Flow(selector)), window))
    } else {
        Err(RequestError::bad_request("start time required"))
    }
}

/// The free-form filter: the raw BPF expression when non-empty, else
/// `None` for all packets in the window.
fn build_filter_or_all(
    audit: &mut AuditContext,
    filter: &Option<String>,
) -> Result<Option<PcapFilter>, RequestError> {
    match present(filter) {
        Some(expression) => {
            audit.filter = expression.to_string();
            Ok(Some(PcapFilter::Expression(expression.to_string())))
        }
        None => {
            audit.filter = "all".to_string();
            Ok(None)
        }
    }
}

/// Derive the flow selector from the event, recording it on the audit
/// line, or fail with 400 `bad-event`.
fn derive_flow(
    audit: &mut AuditContext,
    source: &serde_json::Value,
) -> Result<FlowSelector, RequestError> {
    let selector = pcap::selector_from_event(source)
        .map_err(|err| RequestError::bad_event(err.to_string()))?;
    audit.filter = describe_selector(&selector);
    Ok(selector)
}

/// Download filename: `<sid|flow_id|event>-<epoch>.pcap` for an event
/// request, `capture-<epoch>.pcap` for a standalone one.
fn filename(source: Option<&serde_json::Value>, ts_secs: i64) -> String {
    let id = match source {
        Some(source) => source["alert"]["signature_id"]
            .as_u64()
            .map(|sid| sid.to_string())
            .or_else(|| source["flow_id"].as_u64().map(|id| id.to_string()))
            .unwrap_or_else(|| "event".to_string()),
        None => "capture".to_string(),
    };
    format!("{id}-{ts_secs}.pcap")
}

fn describe_selector(selector: &FlowSelector) -> String {
    let (a_ip, a_port) = selector.a;
    let (b_ip, b_port) = selector.b;
    let port = |p: Option<u16>| p.map(|p| p.to_string()).unwrap_or_default();
    format!(
        "proto={} {}:{} {}:{}",
        selector.proto,
        a_ip,
        port(a_port),
        b_ip,
        port(b_port),
    )
}

// The suite drives the local extraction path end to end, so it is
// compiled out with it on Windows.
#[cfg(all(test, not(windows)))]
mod test {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::task::Poll;

    use axum::body::to_bytes;
    use tokio::sync::Mutex;

    use crate::eventrepo::EventRepo;
    use crate::server::metrics::Metrics;
    use crate::server::pcap::{PcapService, PcapSettings};
    use crate::server::{ServerConfig, ServerContext};
    use crate::sqlite::connection::{ConnectionBuilder, init_event_db};
    use crate::sqlite::eventrepo::SqliteEventRepo;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/pcap/testdata")
            .join(name)
    }

    /// An event whose flow matches the golden fixture spool flow
    /// (udp 10.1.1.5:4000 <-> 192.0.2.10:53, T0=1700000000). The
    /// window covers all four flow packets (offsets 10/20/30/110).
    fn matching_event() -> serde_json::Value {
        json!({
            "timestamp": "2023-11-14T22:15:20.000000+0000",
            "event_type": "alert",
            "proto": "UDP",
            "src_ip": "10.1.1.5",
            "src_port": 4000,
            "dest_ip": "192.0.2.10",
            "dest_port": 53,
            "flow_id": 987654321i64,
            "host": "test-sensor",
            "flow": { "start": "2023-11-14T22:13:20.000000+0000" },
            "alert": { "signature_id": 2000001 },
        })
    }

    async fn build_repo(db_path: &Path) -> EventRepo {
        let builder = ConnectionBuilder::filename(Some(db_path));
        let mut writer = builder.open_connection(true).await.unwrap();
        init_event_db(&mut writer).await.unwrap();
        let pool = builder.open_pool(false).await.unwrap();
        let writer = Arc::new(Mutex::new(writer));
        let repo = SqliteEventRepo::new(writer, pool, Arc::new(Metrics::default()));
        EventRepo::SQLite(repo)
    }

    /// Ingest one event, returning a context configured with the
    /// golden fixture spool.
    async fn context_with_event(
        dir: &Path,
        event: serde_json::Value,
        settings: PcapSettings,
    ) -> Arc<ServerContext> {
        context_with_event_and_optional_source(
            dir,
            event,
            settings,
            Some(PcapSource::Spool(SpoolConfig::new(testdata("spool"), None))),
        )
        .await
    }

    /// As [`context_with_event`], but serving pcap from `spool`.
    async fn context_with_event_and_spool(
        dir: &Path,
        event: serde_json::Value,
        settings: PcapSettings,
        spool: SpoolConfig,
    ) -> Arc<ServerContext> {
        context_with_event_and_source(dir, event, settings, PcapSource::Spool(spool)).await
    }

    /// As [`context_with_event`], but serving pcap from an explicit source.
    async fn context_with_event_and_source(
        dir: &Path,
        event: serde_json::Value,
        settings: PcapSettings,
        source: PcapSource,
    ) -> Arc<ServerContext> {
        context_with_event_and_optional_source(dir, event, settings, Some(source)).await
    }

    /// As [`context_with_event`], but with an optional local capture source.
    async fn context_with_event_and_optional_source(
        dir: &Path,
        event: serde_json::Value,
        settings: PcapSettings,
        source: Option<PcapSource>,
    ) -> Arc<ServerContext> {
        let datastore = build_repo(&dir.join("events.sqlite")).await;
        let mut sink = datastore.get_importer().unwrap();
        sink.submit(event).await.unwrap();
        sink.commit().await.unwrap();

        // A per-test config database file: the in-memory config db
        // (open(None)) uses a process-global shared cache that would
        // collide across parallel tests.
        let configdb = crate::sqlite::configdb::open(Some(&dir.join("config.sqlite")))
            .await
            .unwrap();
        let mut context = ServerContext::new(
            ServerConfig::default(),
            Arc::new(configdb),
            datastore,
            Arc::new(Metrics::default()),
        );
        // The fixture event carries no EveBox agent stamp, so it routes to
        // the local source: source routing remains part of every existing API
        // regression test.
        context.pcap = Arc::new(PcapService::new(settings, source));
        Arc::new(context)
    }

    /// Poll `condition` until it holds, panicking after a deadline.
    async fn wait_for(what: &str, condition: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !condition() {
            assert!(
                std::time::Instant::now() < deadline,
                "not reached within the deadline: {what}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }

    /// True when no extraction holds a global slot — i.e. the
    /// supervisor has released the request it held. Robust to the
    /// concurrency bound: it checks the semaphore is back to its full
    /// complement, not merely that one slot is free.
    fn permits_free(context: &ServerContext) -> bool {
        context.pcap.idle()
    }

    /// A spool of matching flow packets totalling well over the
    /// 16-chunk channel (~1 MiB) plus hyper's buffering, so an
    /// unpolled response body forces the ChannelWriter to park
    /// mid-extraction. Same flow tuple, timestamps, and naming
    /// convention as the golden fixture spool.
    fn write_large_spool(spool: &Path) {
        use crate::pcap::testutil::{ipv4_packet, ports};
        use crate::util::pcap::{create_header_with_snaplen, create_record_raw};
        let mut payload = ports(4000, 53);
        payload.resize(1200, 0xab);
        let frame = ipv4_packet(17, "10.1.1.5", "192.0.2.10", &payload);
        // Link type 1: Ethernet.
        let mut out = create_header_with_snaplen(1, 65_535);
        for i in 0..2500u32 {
            // All within the event's window, in write order.
            out.extend_from_slice(&create_record_raw(
                1_700_000_010,
                i,
                frame.len() as u32,
                &frame,
            ));
        }
        std::fs::write(spool.join("log.pcap.1.1700000000"), out).unwrap();
    }

    fn request(event_id: &str) -> PcapRequestBody {
        PcapRequestBody {
            event_id: Some(event_id.to_string()),
            ..Default::default()
        }
    }

    async fn run(
        context: &Arc<ServerContext>,
        body: PcapRequestBody,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        run_with(context, body, false).await
    }

    /// Drive the native-download path (`GET /api/pcap`), where an empty
    /// result is a valid empty pcap rather than a JSON error.
    async fn run_native(
        context: &Arc<ServerContext>,
        body: PcapRequestBody,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        run_with(context, body, true).await
    }

    async fn run_with(
        context: &Arc<ServerContext>,
        body: PcapRequestBody,
        native: bool,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let response = match handle(context, &body, "tester", "test".to_string(), native).await {
            Ok(response) | Err(response) => response,
        };
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, headers, bytes.to_vec())
    }

    /// Drive the pre-flight (`dry_run`) path — the same routing and
    /// validation as `run`, but stopping before any permit or
    /// extraction and returning the JSON summary body.
    async fn run_validate(
        context: &Arc<ServerContext>,
        body: PcapRequestBody,
    ) -> (StatusCode, Vec<u8>) {
        let response =
            match handle_inner(context, &body, "tester", "test".to_string(), true, false).await {
                Ok(response) | Err(response) => response,
            };
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, bytes.to_vec())
    }

    /// Regenerates the committed golden output from the committed
    /// spool fixture, through the same handler the golden tests use.
    /// Run explicitly after intentional engine or format changes:
    /// cargo test regenerate_golden_fixture -- --ignored
    #[tokio::test]
    #[ignore]
    async fn regenerate_golden_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!body.is_empty());
        std::fs::write(testdata("expected.pcap"), &body).unwrap();
    }

    #[tokio::test]
    async fn happy_path_matches_golden_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let (status, headers, body) = run(&context, request("1")).await;

        assert_eq!(
            status,
            StatusCode::OK,
            "body={:?}",
            String::from_utf8_lossy(&body)
        );
        assert_eq!(
            headers.get(CONTENT_TYPE).unwrap(),
            "application/vnd.tcpdump.pcap"
        );
        let disposition = headers.get(CONTENT_DISPOSITION).unwrap().to_str().unwrap();
        // Filename keys on the alert signature id.
        assert!(
            disposition.contains("2000001-"),
            "disposition={disposition}"
        );
        assert!(disposition.ends_with(".pcap"), "disposition={disposition}");

        let expected = std::fs::read(testdata("expected.pcap")).unwrap();
        assert_eq!(body, expected);

        // The supervisor has settled once the permit becomes free.
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    #[tokio::test]
    async fn explicit_file_source_backs_event_download() {
        let dir = tempfile::tempdir().unwrap();
        let capture = testdata("spool/log.pcap.1.1700000000");
        let context = context_with_event_and_source(
            dir.path(),
            matching_event(),
            PcapSettings::default(),
            PcapSource::Files(vec![capture]),
        )
        .await;
        let (status, headers, body) = run(&context, request("1")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get(CONTENT_TYPE).unwrap(),
            "application/vnd.tcpdump.pcap"
        );
        assert!(!body.is_empty());
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    #[tokio::test]
    async fn truncated_before_first_packet_returns_empty_200() {
        let dir = tempfile::tempdir().unwrap();
        // A byte cap below a single packet: extraction succeeds with
        // zero output and the truncated marker, not a failure.
        let settings = PcapSettings {
            max_bytes: 30,
            ..Default::default()
        };
        let context = context_with_event(dir.path(), matching_event(), settings).await;
        let (status, headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
        assert_eq!(headers.get("x-evebox-pcap-truncated").unwrap(), "true");
        assert_eq!(
            headers.get(CONTENT_TYPE).unwrap(),
            "application/vnd.tcpdump.pcap"
        );
        assert!(headers.get(CONTENT_DISPOSITION).is_some());
        // The supervisor still joins the producer after truncation.
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    #[tokio::test]
    async fn no_match_returns_404() {
        let dir = tempfile::tempdir().unwrap();
        // Same sensor/window, but a flow that appears in no packet.
        let mut event = matching_event();
        event["dest_ip"] = json!("203.0.113.9");
        let context = context_with_event(dir.path(), event, PcapSettings::default()).await;
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "no-match");
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    #[tokio::test]
    async fn no_candidate_files_returns_404() {
        let dir = tempfile::tempdir().unwrap();
        // A window years before any fixture file exists.
        let mut event = matching_event();
        event["timestamp"] = json!("2020-01-01T00:00:00.000000+0000");
        event["flow"]["start"] = json!("2020-01-01T00:00:00.000000+0000");
        let context = context_with_event(dir.path(), event, PcapSettings::default()).await;
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "no-candidate-files");
    }

    #[tokio::test]
    async fn native_no_match_returns_empty_pcap() {
        let dir = tempfile::tempdir().unwrap();
        // Same no-match flow as above, but on the native-download path
        // the browser cannot read a JSON error, so it gets a valid empty
        // pcap (200) with a download filename instead.
        let mut event = matching_event();
        event["dest_ip"] = json!("203.0.113.9");
        let context = context_with_event(dir.path(), event, PcapSettings::default()).await;
        let (status, headers, body) = run_native(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get(CONTENT_DISPOSITION).is_some());
        assert_eq!(body, empty_pcap_bytes());
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    #[tokio::test]
    async fn native_no_candidate_files_returns_empty_pcap() {
        let dir = tempfile::tempdir().unwrap();
        let mut event = matching_event();
        event["timestamp"] = json!("2020-01-01T00:00:00.000000+0000");
        event["flow"]["start"] = json!("2020-01-01T00:00:00.000000+0000");
        let context = context_with_event(dir.path(), event, PcapSettings::default()).await;
        let (status, headers, body) = run_native(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get(CONTENT_DISPOSITION).is_some());
        assert_eq!(body, empty_pcap_bytes());
    }

    #[tokio::test]
    async fn unstamped_event_host_does_not_affect_local_spool() {
        // An event without an EveBox agent stamp was ingested by this
        // server, so the local spool serves it whatever its host says.
        let dir = tempfile::tempdir().unwrap();
        let mut event = matching_event();
        event["host"] = json!("some-other-sensor");
        let context = context_with_event(dir.path(), event, PcapSettings::default()).await;
        let (status, _headers, _body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    #[tokio::test]
    async fn stamped_event_does_not_fall_through_to_local_spool() {
        // An event stamped by an EveBox agent must be served by that agent;
        // with the agent gone, routing must not silently use the local spool.
        let dir = tempfile::tempdir().unwrap();
        let mut event = matching_event();
        event["evebox"] = json!({ "agent": { "hostname": "gone-host" } });
        let context = context_with_event(dir.path(), event, PcapSettings::default()).await;
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "no-source");
    }

    #[tokio::test]
    async fn missing_event_returns_404() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let (status, _headers, body) = run(&context, request("9999")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "event-not-found");
    }

    #[tokio::test]
    async fn global_cap_returns_429() {
        let dir = tempfile::tempdir().unwrap();
        let settings = PcapSettings {
            max_concurrent: 1,
            ..Default::default()
        };
        let context = context_with_event(dir.path(), matching_event(), settings).await;
        // Hold the only global slot so the request cannot acquire one.
        let _permit = context.pcap.try_acquire().unwrap();
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "busy");
    }

    #[tokio::test]
    async fn bad_event_returns_400() {
        let dir = tempfile::tempdir().unwrap();
        // An event with no addresses cannot yield a flow selector.
        let mut event = matching_event();
        event.as_object_mut().unwrap().remove("src_ip");
        let context = context_with_event(dir.path(), event, PcapSettings::default()).await;
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "bad-event");
    }

    #[tokio::test]
    async fn unreadable_spool_returns_502() {
        let dir = tempfile::tempdir().unwrap();
        // A spool directory that cannot be read fails the fetch with
        // an io error before any output byte: 502.
        let context = context_with_event_and_spool(
            dir.path(),
            matching_event(),
            PcapSettings::default(),
            SpoolConfig::new(dir.path().join("missing-spool"), None),
        )
        .await;
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "io");
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// The 504 first-byte timeout with a fetch that never
    /// acknowledges the cancel: the fetch task is deterministically
    /// wedged by saturating a one-thread blocking pool, so the
    /// handler sees exactly what a fetch stuck in a blocking read
    /// looks like — no first frame, an open channel, an incomplete
    /// task. The handler must reply without awaiting that task; the
    /// supervisor waits out its (tiny) grace, logs outcome=wedged,
    /// and releases the permits anyway.
    #[test]
    fn first_byte_timeout_returns_504() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let settings = PcapSettings {
                request_timeout: std::time::Duration::from_millis(1),
                wedge_grace: std::time::Duration::from_millis(100),
                ..Default::default()
            };
            let context = context_with_event(dir.path(), matching_event(), settings).await;

            // Wedge the only blocking-pool thread; the fetch task
            // queues behind it and can never start.
            let (wedge_tx, wedge_rx) = std::sync::mpsc::channel::<()>();
            let wedge = tokio::task::spawn_blocking(move || {
                let _ = wedge_rx.recv();
            });

            // Awaiting the wedged task would hang, not 504.
            let (status, _headers, body) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                run(&context, request("1")),
            )
            .await
            .expect("the 504 must not wait for the wedged fetch");
            assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["error"]["code"], "timeout");

            // The supervisor gives the (never-started) fetch its
            // grace, then declares it wedged and releases the permits
            // with the thread still out there.
            wait_for("wedged supervisor settled", || permits_free(&context)).await;

            // Unwedge the detached fetch so it can see the cancellation.
            wedge_tx.send(()).unwrap();
            let _ = wedge.await;
        });
    }

    /// The 504 with a fetch that CAN acknowledge the cancel: once the
    /// wedge clears, the fetch stops within the (default) grace and
    /// the supervisor logs outcome=timeout and releases the permits.
    #[test]
    fn timeout_with_responsive_fetch_releases_permits() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let settings = PcapSettings {
                request_timeout: std::time::Duration::from_millis(1),
                ..Default::default()
            };
            let context = context_with_event(dir.path(), matching_event(), settings).await;

            let (wedge_tx, wedge_rx) = std::sync::mpsc::channel::<()>();
            let wedge = tokio::task::spawn_blocking(move || {
                let _ = wedge_rx.recv();
            });

            let (status, _headers, _body) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                run(&context, request("1")),
            )
            .await
            .expect("the 504 must not wait for the wedged fetch");
            assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);

            // Unwedge: the fetch starts, observes the cancel on its
            // first checkpoint, and the supervisor settles within the
            // grace.
            wedge_tx.send(()).unwrap();
            let _ = wedge.await;
            wait_for("timeout supervisor settled", || permits_free(&context)).await;
        });
    }

    /// A client abort BEFORE the first byte — the request future
    /// dropped while the fetch was still working — must cancel the
    /// fetch via the pre-stream guard and free both permits. Confirmed missing before this
    /// guard existed: the fetch ran to its deadline with no audit
    /// line. The blocking pool is wedged so the drop happens
    /// deterministically before any output.
    #[test]
    fn pre_first_byte_disconnect_cancels_and_releases_permits() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let context =
                context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;

            // Wedge the only blocking-pool thread so no frame can
            // arrive while the request future is still alive.
            let (wedge_tx, wedge_rx) = std::sync::mpsc::channel::<()>();
            let wedge = tokio::task::spawn_blocking(move || {
                let _ = wedge_rx.recv();
            });

            let body = request("1");
            let mut fut = Box::pin(handle(&context, &body, "tester", "test".to_string(), false));
            // Drive the handler until the extraction is spawned: the
            // poll that takes the permits parks on the first frame.
            loop {
                if let Poll::Ready(response) = futures::poll!(fut.as_mut()) {
                    let response = match response {
                        Ok(response) | Err(response) => response,
                    };
                    panic!(
                        "handler finished before the disconnect: {}",
                        response.status()
                    );
                }
                if !permits_free(&context) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }

            // The disconnect: dropping the request future must cancel
            // the fetch promptly through the pre-stream guard.
            drop(fut);

            // Unwedge so the queued, already-cancelled fetch can run
            // and the supervisor can settle.
            wedge_tx.send(()).unwrap();
            let _ = wedge.await;

            wait_for("aborted extraction settled and permits released", || {
                permits_free(&context)
            })
            .await;
        });
    }

    /// The permits are keyed to the extraction, not to the browser
    /// draining the body: with the
    /// tiny fixture the fetch completes immediately, and both permits
    /// must come free while the 200's body is still completely unpolled.
    #[tokio::test]
    async fn local_permits_release_while_body_unpolled() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let response =
            match handle(&context, &request("1"), "tester", "test".to_string(), false).await {
                Ok(response) => response,
                Err(response) => panic!("expected a 200, got {}", response.status()),
            };
        assert_eq!(response.status(), StatusCode::OK);

        wait_for("permits released with the body unpolled", || {
            permits_free(&context)
        })
        .await;
        drop(response);
    }

    /// A client that stops reading while HOLDING the connection open
    /// must not pin the permits until TCP death: the channel fills,
    /// the ChannelWriter push times out and aborts the fetch, and the
    /// supervisor frees both permits — all while the response, and so the
    /// channel's receiver, is still alive and unpolled.
    #[tokio::test]
    async fn stalled_client_releases_permits() {
        let dir = tempfile::tempdir().unwrap();
        let spool_dir = dir.path().join("spool");
        std::fs::create_dir(&spool_dir).unwrap();
        write_large_spool(&spool_dir);
        let settings = PcapSettings {
            // Tightened so the test reaps quickly; request_timeout
            // stays at its 60s default so neither the first-byte 504
            // nor the engine deadline can fire first.
            stall_timeout: std::time::Duration::from_millis(300),
            ..Default::default()
        };
        let context = context_with_event_and_spool(
            dir.path(),
            matching_event(),
            settings,
            SpoolConfig::new(spool_dir, None),
        )
        .await;
        let response =
            match handle(&context, &request("1"), "tester", "test".to_string(), false).await {
                Ok(response) => response,
                Err(response) => panic!("expected a 200, got {}", response.status()),
            };
        assert_eq!(response.status(), StatusCode::OK);

        wait_for("stalled extraction reaped and permits released", || {
            permits_free(&context)
        })
        .await;
        drop(response);
    }

    /// A browser disconnect (response body dropped) must cancel the
    /// fetch and free the permits well before any stall timeout: the
    /// stall bound here is 60s but the permits must come free within
    /// the 5s polling deadline, so only the cancellation path can
    /// pass this test.
    #[tokio::test]
    async fn disconnect_cancels_fetch_and_releases_permits() {
        let dir = tempfile::tempdir().unwrap();
        let spool_dir = dir.path().join("spool");
        std::fs::create_dir(&spool_dir).unwrap();
        write_large_spool(&spool_dir);
        let context = context_with_event_and_spool(
            dir.path(),
            matching_event(),
            PcapSettings::default(),
            SpoolConfig::new(spool_dir, None),
        )
        .await;
        let response =
            match handle(&context, &request("1"), "tester", "test".to_string(), false).await {
                Ok(response) => response,
                Err(response) => panic!("expected a 200, got {}", response.status()),
            };
        assert_eq!(response.status(), StatusCode::OK);
        drop(response);

        wait_for(
            "disconnected extraction cancelled and permits released",
            || permits_free(&context),
        )
        .await;
    }

    /// Fix 1 anti-regression: a slow-but-LIVE client — one that keeps
    /// draining, just slowly, so the whole transfer runs past
    /// `request_timeout` AND past the watchdog's wall-clock
    /// `idle_bound` — must complete successfully.
    /// Every drained chunk lets a parked `send_frame` complete and
    /// advance the progress stamp, so the idle watchdog never mistakes
    /// healthy backpressure for a wedge. A naive
    /// `timeout(idle_bound, handle)` would wrongly reap this healthy
    /// download as wedged; the progress-aware watchdog must not.
    #[tokio::test]
    async fn slow_but_live_client_completes_ok_not_wedged() {
        use futures::StreamExt;
        let dir = tempfile::tempdir().unwrap();
        let spool_dir = dir.path().join("spool");
        std::fs::create_dir(&spool_dir).unwrap();
        write_large_spool(&spool_dir);
        // Tiny bounds: the dribbled drain below runs far longer than
        // any single bound and longer than their sum (idle_bound =
        // request_timeout + stall_timeout + grace = 1100ms).
        // request_timeout stays generous enough that the first frame
        // beats the first-byte deadline even under a loaded machine.
        let settings = PcapSettings {
            request_timeout: std::time::Duration::from_millis(500),
            stall_timeout: std::time::Duration::from_millis(300),
            wedge_grace: std::time::Duration::from_millis(300),
            ..Default::default()
        };
        let context = context_with_event_and_spool(
            dir.path(),
            matching_event(),
            settings,
            SpoolConfig::new(spool_dir, None),
        )
        .await;
        let response =
            match handle(&context, &request("1"), "tester", "test".to_string(), false).await {
                Ok(response) => response,
                Err(response) => panic!("expected a 200, got {}", response.status()),
            };
        assert_eq!(response.status(), StatusCode::OK);

        // Dribble the body: one chunk per interval, each interval well
        // under stall_timeout so the parked producer send always
        // completes (progress keeps advancing), and the total drain
        // (~3 MiB in 64 KiB chunks at 40ms each ≈ 2s) far exceeds
        // idle_bound (1100ms).
        let mut stream = response.into_body().into_data_stream();
        let mut total = 0usize;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("a slow-but-live body must not error");
            total += chunk.len();
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        }
        assert!(total > 0, "the slow client still received the capture");

        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// Fix 1 wedge reap: a producer stuck in a non-returning read
    /// AFTER its first chunk — client connected but idle, channel
    /// empty — hands off no further frame, so `last_progress` freezes
    /// and the idle watchdog reaps it (None) near `idle_bound`, without
    /// the token ever being cancelled. Reproduces the stuck-read case
    /// faithfully through the real `await_outcome` loop and a real
    /// `ChannelWriter`: the producer emits one frame (advancing
    /// progress) then blocks on a release the test controls.
    #[test]
    fn wedged_producer_after_first_chunk_reaps_as_wedged() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let (tx, mut rx) = mpsc::channel::<Frame>(16);
            let started = std::time::Instant::now();
            let last_progress = Arc::new(AtomicU64::new(0));
            let cancel = CancellationToken::new();
            // Stands in for a stuck read syscall: holds the producer
            // wedged after its first send until the test releases it.
            let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
            let writer_progress = last_progress.clone();
            let mut handle = tokio::task::spawn_blocking(move || {
                let mut writer = ChannelWriter {
                    tx,
                    buf: Vec::with_capacity(CHUNK_SIZE),
                    cancel: CancellationToken::new(),
                    stall_timeout: std::time::Duration::from_secs(60),
                    started,
                    last_progress: writer_progress,
                };
                // First chunk goes out and advances progress.
                writer
                    .send_frame(Frame::Data(Bytes::from_static(b"first")))
                    .unwrap();
                // Wedge: block forever (until released), emitting
                // nothing more — no engine checkpoint, and no writer
                // stall because the channel is empty.
                let _ = release_rx.recv();
                Ok::<_, FetchError>(FetchStats::default())
            });

            // The client drains the first chunk, then stays connected
            // but idle: rx is held (alive) and never polled again.
            let first = rx.recv().await;
            assert!(matches!(first, Some(Frame::Data(_))));

            let idle_bound = std::time::Duration::from_millis(300);
            let check_interval = std::time::Duration::from_millis(50);
            let grace = std::time::Duration::from_millis(100);
            let waited = std::time::Instant::now();
            let outcome = await_outcome(
                &mut handle,
                &cancel,
                started,
                &last_progress,
                idle_bound,
                check_interval,
                grace,
            )
            .await;
            assert!(
                outcome.is_none(),
                "a producer with no output progress must reap as wedged"
            );
            // Reaped near idle_bound, not left to hang.
            assert!(waited.elapsed() < std::time::Duration::from_secs(2));
            // A watchdog reap, not a client disconnect: the token was
            // never cancelled.
            assert!(!cancel.is_cancelled());

            // Release the wedged producer so the runtime can drain, and
            // keep rx alive until now to model the connected client.
            drop(rx);
            let _ = release_tx.send(());
            let _ = handle.await;
        });
    }

    /// The stall bound on the writer itself: a full channel whose
    /// receiver is alive but never drained must fail the push with
    /// TimedOut after the stall timeout instead of parking forever.
    #[tokio::test]
    async fn channel_writer_stall_timeout_errors_when_not_drained() {
        let (tx, _rx) = mpsc::channel::<Frame>(1);
        let mut writer = ChannelWriter {
            tx,
            buf: Vec::new(),
            cancel: CancellationToken::new(),
            stall_timeout: std::time::Duration::from_millis(100),
            started: std::time::Instant::now(),
            last_progress: Arc::new(AtomicU64::new(0)),
        };
        let chunk = vec![0u8; CHUNK_SIZE];
        // Fills the only channel slot.
        writer.write_all(&chunk).unwrap();
        // Cannot be pushed: the channel is full and nobody drains it.
        let err = writer.write_all(&chunk).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    }

    /// A cancelled token aborts a parked send without waiting for the
    /// stall timeout.
    #[tokio::test]
    async fn channel_writer_cancel_aborts_parked_send() {
        let (tx, _rx) = mpsc::channel::<Frame>(1);
        let cancel = CancellationToken::new();
        let mut writer = ChannelWriter {
            tx,
            buf: Vec::new(),
            cancel: cancel.clone(),
            stall_timeout: std::time::Duration::from_secs(60),
            started: std::time::Instant::now(),
            last_progress: Arc::new(AtomicU64::new(0)),
        };
        let chunk = vec![0u8; CHUNK_SIZE];
        writer.write_all(&chunk).unwrap();
        cancel.cancel();
        let started = std::time::Instant::now();
        let err = writer.write_all(&chunk).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }

    #[test]
    fn disconnect_guard_cancels_only_when_armed() {
        // Dropped armed (body dropped mid-stream): cancels as a
        // client abort.
        let cancel = CancellationToken::new();
        let reason = CancelReason::new();
        let guard = DisconnectGuard {
            reason: reason.clone(),
            cancel: cancel.clone(),
            done: false,
        };
        drop(guard);
        assert!(cancel.is_cancelled());
        assert_eq!(reason.get(), CancelCause::Client);

        // Disarmed (body streamed to completion): no cancel.
        let cancel = CancellationToken::new();
        let reason = CancelReason::new();
        let mut guard = DisconnectGuard {
            reason: reason.clone(),
            cancel: cancel.clone(),
            done: false,
        };
        guard.disarm();
        drop(guard);
        assert!(!cancel.is_cancelled());
        assert_eq!(reason.get(), CancelCause::None);
    }

    #[test]
    fn cancel_reason_first_cause_wins() {
        let reason = CancelReason::new();
        assert_eq!(reason.get(), CancelCause::None);
        reason.set(CancelCause::Timeout);
        reason.set(CancelCause::Client);
        assert_eq!(reason.get(), CancelCause::Timeout);
    }

    fn body_state(rx: mpsc::Receiver<Frame>, cancel: &CancellationToken) -> BodyState {
        BodyState {
            rx,
            guard: DisconnectGuard {
                reason: CancelReason::new(),
                cancel: cancel.clone(),
                done: false,
            },
            queued: VecDeque::from([Bytes::from_static(b"first")]),
            finished: false,
            completion: None,
        }
    }

    /// A producer failure after streaming began must surface as a
    /// body stream error — an aborted transfer — never a clean
    /// chunked EOF that lets the client save a truncated pcap as
    /// complete.
    #[tokio::test]
    async fn mid_stream_producer_failure_errors_the_body() {
        let (tx, rx) = mpsc::channel::<Frame>(16);
        tx.try_send(Frame::Data(Bytes::from_static(b" more")))
            .unwrap();
        tx.try_send(Frame::Done(ProducerEnd::Io)).unwrap();
        let cancel = CancellationToken::new();
        let body = Body::from_stream(body_stream(body_state(rx, &cancel)));
        let result = to_bytes(body, usize::MAX).await;
        assert!(
            result.is_err(),
            "a failed extraction must not end the body cleanly"
        );
        // The terminal frame disarmed the guard: this is a producer
        // failure, not a client disconnect.
        assert!(!cancel.is_cancelled());
    }

    /// The channel closing without a terminal frame (producer panic,
    /// or a Done dropped on a stalled client) also errors the body.
    #[tokio::test]
    async fn channel_closed_without_done_errors_the_body() {
        let (tx, rx) = mpsc::channel::<Frame>(16);
        drop(tx);
        let cancel = CancellationToken::new();
        let body = Body::from_stream(body_stream(body_state(rx, &cancel)));
        assert!(to_bytes(body, usize::MAX).await.is_err());
        assert!(!cancel.is_cancelled());
    }

    /// Producer success — including deliberate limit-truncation —
    /// ends the body cleanly with every queued and streamed chunk
    /// delivered.
    #[tokio::test]
    async fn body_stream_ends_cleanly_on_success() {
        let (tx, rx) = mpsc::channel::<Frame>(16);
        tx.try_send(Frame::Data(Bytes::from_static(b" second")))
            .unwrap();
        tx.try_send(Frame::Done(ProducerEnd::Complete { truncated: true }))
            .unwrap();
        let cancel = CancellationToken::new();
        let state = body_state(rx, &cancel);
        let body = Body::from_stream(body_stream(state));
        let bytes = to_bytes(body, usize::MAX).await.unwrap();
        assert_eq!(bytes.as_ref(), b"first second");
        assert!(!cancel.is_cancelled());
    }

    /// The final flush's error must propagate out of the producer: a
    /// client that stalls at the very tail — fetch complete, tail
    /// chunk unsendable — returns TimedOut instead of swallowing the
    /// flush error and reporting outcome=ok.
    #[test]
    fn tail_flush_stall_propagates_timedout() {
        let (tx, _rx) = mpsc::channel::<Frame>(1);
        // Fill the only slot so the tail flush can never complete.
        tx.try_send(Frame::Data(Bytes::new())).unwrap();
        let cancel = CancellationToken::new();
        let mut writer = ChannelWriter {
            tx,
            buf: Vec::with_capacity(CHUNK_SIZE),
            cancel: cancel.clone(),
            stall_timeout: std::time::Duration::from_millis(50),
            started: std::time::Instant::now(),
            last_progress: Arc::new(AtomicU64::new(0)),
        };
        // The whole fixture output is far below one chunk, so all of
        // it rides on the final flush.
        let spool = SpoolConfig::new(testdata("spool"), None);
        let request = PcapRequest::default();
        let result = run_extraction(&PcapSource::Spool(spool), &request, &mut writer, &cancel);
        match result {
            Err(FetchError::Io(err)) => assert_eq!(err.kind(), std::io::ErrorKind::TimedOut),
            other => panic!("expected a TimedOut io error, got {other:?}"),
        }
    }

    /// A throwaway writer over a fresh channel, for `finish_producer`
    /// unit tests that supply their own tx/rx.
    fn done_test_writer(tx: mpsc::Sender<Frame>) -> ChannelWriter {
        ChannelWriter {
            tx,
            buf: Vec::with_capacity(CHUNK_SIZE),
            cancel: CancellationToken::new(),
            stall_timeout: std::time::Duration::from_millis(50),
            started: std::time::Instant::now(),
            last_progress: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Fix 2: a fetch that SUCCEEDS but whose terminal Done frame
    /// cannot be delivered — the client stalled at the very tail with
    /// the channel full after the final data flush already landed —
    /// must be downgraded to a TimedOut io error. Left as ok, the
    /// supervisor would log outcome=ok while the client's body ends
    /// without a Done and hyper tears the download.
    #[tokio::test]
    async fn done_send_stall_downgrades_success_to_failure() {
        let (tx, _rx) = mpsc::channel::<Frame>(1);
        // Fill the only slot so the Done send can never complete; the
        // receiver stays alive, so the send stalls (TimedOut) rather
        // than breaking (BrokenPipe).
        tx.try_send(Frame::Data(Bytes::new())).unwrap();
        let mut writer = done_test_writer(tx);
        let result = finish_producer(Ok(FetchStats::default()), &mut writer);
        match result {
            Err(FetchError::Io(err)) => assert_eq!(err.kind(), std::io::ErrorKind::TimedOut),
            other => {
                panic!("a stalled Done send must downgrade success to TimedOut, got {other:?}")
            }
        }
    }

    /// A fetch that succeeds and whose Done frame IS delivered keeps
    /// its ok result: the downgrade must never touch the normal
    /// success path.
    #[tokio::test]
    async fn done_send_success_keeps_ok() {
        let (tx, mut rx) = mpsc::channel::<Frame>(16);
        let mut writer = done_test_writer(tx);
        let result = finish_producer(Ok(FetchStats::default()), &mut writer);
        assert!(result.is_ok(), "a delivered Done keeps the ok result");
        assert!(
            matches!(rx.try_recv(), Ok(Frame::Done(_))),
            "the terminal Done frame reached the channel"
        );
    }

    /// A fetch that ALREADY failed for a client-caused reason skips the
    /// Done send entirely — nobody is reading — and returns unchanged:
    /// the downgrade must not double-handle a client-caused error.
    #[tokio::test]
    async fn finish_producer_skips_done_when_client_already_gone() {
        let (tx, rx) = mpsc::channel::<Frame>(1);
        drop(rx); // Receiver gone: any send would fail BrokenPipe.
        let mut writer = done_test_writer(tx);
        let result = finish_producer(
            Err(FetchError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "pcap receiver dropped",
            ))),
            &mut writer,
        );
        match result {
            Err(FetchError::Io(err)) => assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe),
            other => panic!("a client-gone error must pass through unchanged, got {other:?}"),
        }
    }

    /// Drive a `handle()` future to completion without ever polling
    /// it while the extraction is mid-flight: the poll that spawns
    /// the extraction parks on the first frame with the permits held,
    /// and the next poll happens only after the supervisor released
    /// them — so the data frame and the terminal frame are both
    /// observable in that poll. This makes the best-effort early-Done
    /// path deterministic for tests.
    async fn drive_race_free(context: &Arc<ServerContext>, body: PcapRequestBody) -> Response {
        let mut fut = Box::pin(handle(context, &body, "tester", "test".to_string(), false));
        loop {
            if !permits_free(context) {
                wait_for("extraction finished", || permits_free(context)).await;
                match futures::poll!(fut.as_mut()) {
                    Poll::Ready(response) => {
                        return match response {
                            Ok(response) | Err(response) => response,
                        };
                    }
                    Poll::Pending => panic!("handler still pending after the extraction finished"),
                }
            }
            if let Poll::Ready(response) = futures::poll!(fut.as_mut()) {
                return match response {
                    Ok(response) | Err(response) => response,
                };
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    }

    /// A single-chunk truncated output must carry the truncated
    /// marker AND the partial body: the terminal frame travels right
    /// behind the only data chunk, so the outcome is known before the
    /// headers commit.
    #[tokio::test]
    async fn truncated_single_chunk_sets_header_with_partial_body() {
        let dir = tempfile::tempdir().unwrap();
        let expected = std::fs::read(testdata("expected.pcap")).unwrap();
        // One byte short of the full output: at least one packet
        // fits, the last one does not.
        let settings = PcapSettings {
            max_bytes: (expected.len() - 1) as u64,
            ..Default::default()
        };
        let context = context_with_event(dir.path(), matching_event(), settings).await;
        let response = drive_race_free(&context, request("1")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-evebox-pcap-truncated").unwrap(),
            "true"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(!body.is_empty(), "the partial capture must be delivered");
        assert!(body.len() < expected.len());
        assert!(expected.starts_with(body.as_ref()));
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// The POST wrapper must wait for the terminal frame before committing
    /// headers when truncation follows multiple data frames. Otherwise the
    /// browser saves the partial capture without showing its truncation toast.
    #[tokio::test]
    async fn truncated_multi_chunk_post_sets_header_with_partial_body() {
        let dir = tempfile::tempdir().unwrap();
        let spool = dir.path().join("spool");
        std::fs::create_dir(&spool).unwrap();
        write_large_spool(&spool);
        let max_bytes = (CHUNK_SIZE * 3) as u64;
        let settings = PcapSettings {
            max_bytes,
            ..Default::default()
        };
        let context = context_with_event_and_spool(
            dir.path(),
            matching_event(),
            settings,
            SpoolConfig::new(spool, None),
        )
        .await;

        let session = Arc::new(crate::server::session::Session::anonymous(Some(
            "tester".to_string(),
        )));
        let response = post_pcap(
            State(context.clone()),
            SessionExtractor(session),
            Extension(ConnectInfo("127.0.0.1:12345".parse().unwrap())),
            HeaderMap::new(),
            Json(request("1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-evebox-pcap-truncated").unwrap(),
            "true"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(body.len() > CHUNK_SIZE, "fixture must span multiple chunks");
        assert!(body.len() as u64 <= max_bytes);
    }

    /// The buffered POST endpoint must never honor a caller-controlled cap
    /// above its fixed quick-download limit: doing so would make
    /// `buffer_post_body` aggregate an arbitrarily large response in RAM.
    /// Large and unlimited captures remain available through native GET.
    #[tokio::test]
    async fn buffered_post_rejects_max_size_above_default() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let session = Arc::new(crate::server::session::Session::anonymous(Some(
            "tester".to_string(),
        )));
        let body = PcapRequestBody {
            event_id: Some("1".to_string()),
            max_size: Some("unlimited".to_string()),
            ..Default::default()
        };

        let response = post_pcap(
            State(context.clone()),
            SessionExtractor(session),
            Extension(ConnectInfo("127.0.0.1:12345".parse().unwrap())),
            HeaderMap::new(),
            Json(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "max-size-too-large");
        assert!(permits_free(&context));
    }

    /// A backlog of detached extraction threads (a wedged spool
    /// accumulating workers whose permits were already released)
    /// gates new requests with a 503 before spawning more work.
    #[tokio::test]
    async fn extraction_backlog_returns_503_wedged() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        // The backstop is max_concurrent (16): a backlog of double
        // that reaches the 2x gate.
        context
            .pcap
            .inflight
            .store(32, std::sync::atomic::Ordering::SeqCst);
        let (status, _headers, body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "wedged");
        assert!(permits_free(&context));
    }

    /// The full `post_pcap` entry point returns the golden capture.
    #[tokio::test]
    async fn post_pcap_returns_golden_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let session = Arc::new(crate::server::session::Session::anonymous(Some(
            "tester".to_string(),
        )));
        let response = post_pcap(
            State(context.clone()),
            SessionExtractor(session),
            Extension(ConnectInfo("127.0.0.1:12345".parse().unwrap())),
            HeaderMap::new(),
            Json(request("1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let expected = std::fs::read(testdata("expected.pcap")).unwrap();
        assert_eq!(body.to_vec(), expected);
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    // The fixture flow packets sit at 1700000000 + {10, 20, 30, 110}
    // seconds; the flow start is 2023-11-14T22:13:20Z (1700000000).
    const FIXTURE_START: &str = "2023-11-14T22:13:20+00:00";

    /// Event-relative mode narrows the window around the event
    /// timestamp and keeps the derived flow: a ±1m window catches only
    /// the last flow packet, so its output is a strict subset of the
    /// default mode's full-flow output.
    #[tokio::test]
    async fn event_relative_window_narrows_and_uses_flow() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;

        // Default mode: the whole derived flow window (all four packets).
        let (status, _headers, default_body) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!default_body.is_empty());
        wait_for("supervisor settled", || permits_free(&context)).await;

        // Event-relative ±1m: window [ts-60, ts+60] covers only the
        // last flow packet, a strict subset of the default output.
        let relative = PcapRequestBody {
            event_id: Some("1".to_string()),
            before: Some("1m".to_string()),
            after: Some("1m".to_string()),
            ..Default::default()
        };
        let (status, _headers, relative_body) = run(&context, relative).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "body={:?}",
            String::from_utf8_lossy(&relative_body)
        );
        assert!(
            !relative_body.is_empty(),
            "the relative window still matched"
        );
        assert!(
            relative_body.len() < default_body.len(),
            "the ±1m window must extract fewer bytes than the full flow \
             (relative={}, default={})",
            relative_body.len(),
            default_body.len()
        );
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// Free-form mode with a raw BPF filter and an explicit
    /// start/duration extracts via `PcapFilter::Expression` over the
    /// custom window; the event provides only its identifier and
    /// filename metadata.
    #[tokio::test]
    async fn freeform_raw_filter_extracts() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            event_id: Some("1".to_string()),
            filter: Some("udp and port 53".to_string()),
            start: Some(FIXTURE_START.to_string()),
            duration: Some("5m".to_string()),
            ..Default::default()
        };
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "body={:?}",
            String::from_utf8_lossy(&out)
        );
        assert!(!out.is_empty(), "the raw filter matched the flow packets");
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// A native GET request may lift the server's configured cap: with a
    /// small default the buffered quick response is truncated, but the
    /// same request with `max_size: unlimited` streams the full flow.
    #[tokio::test]
    async fn native_max_size_overrides_default() {
        let dir = tempfile::tempdir().unwrap();
        let spool = dir.path().join("spool");
        std::fs::create_dir_all(&spool).unwrap();
        write_large_spool(&spool);
        let settings = PcapSettings {
            max_bytes: 60_000,
            ..PcapSettings::default()
        };
        let context = context_with_event_and_spool(
            dir.path(),
            matching_event(),
            settings,
            SpoolConfig::new(spool.clone(), None),
        )
        .await;

        // The small server default caps the output well below the full
        // ~3 MB flow.
        let (status, _headers, capped) = run(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            capped.len() <= 60_000,
            "server default caps output: {}",
            capped.len()
        );
        wait_for("supervisor settled", || permits_free(&context)).await;

        // The per-request override lifts the cap, so the full flow is
        // returned — many times larger than the capped output.
        let body = PcapRequestBody {
            event_id: Some("1".to_string()),
            max_size: Some("unlimited".to_string()),
            ..Default::default()
        };
        let (status, _headers, full) = run_native(&context, body).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            full.len() > capped.len(),
            "override lifts the cap: full={} capped={}",
            full.len(),
            capped.len()
        );
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// A malformed `max_size` fails up front with `bad-max-size`
    /// without taking a permit.
    #[tokio::test]
    async fn bad_max_size_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            event_id: Some("1".to_string()),
            max_size: Some("not-a-size".to_string()),
            ..Default::default()
        };
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "bad-max-size");
        assert!(permits_free(&context));
    }

    /// The pre-flight validates and reports the filename without
    /// taking a permit or extracting any packets.
    #[tokio::test]
    async fn validate_reports_without_extracting() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let (status, out) = run_validate(&context, request("1")).await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["ok"], true);
        assert!(
            json["filename"].as_str().unwrap().starts_with("2000001-"),
            "filename derived from the event: {}",
            json["filename"]
        );
        assert!(permits_free(&context));
    }

    /// A failing pre-flight returns the structured error. A BPF filter
    /// is deliberately NOT validated here — only the compiling side can
    /// reject one — so the probe uses a bad duration.
    #[tokio::test]
    async fn validate_error_returns_structured_error() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            event_id: Some("1".to_string()),
            start: Some(FIXTURE_START.to_string()),
            duration: Some("not-a-duration".to_string()),
            ..Default::default()
        };
        let (status, out) = run_validate(&context, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "bad-request");
    }

    /// Standalone free-form (no event_id) with start + duration and no
    /// filter extracts all packets in the window.
    #[tokio::test]
    async fn standalone_freeform_all_packets() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            start: Some(FIXTURE_START.to_string()),
            duration: Some("5m".to_string()),
            ..Default::default()
        };
        let (status, headers, out) = run(&context, body).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "body={:?}",
            String::from_utf8_lossy(&out)
        );
        // A standalone capture filename keys on "capture", not an event id.
        let disposition = headers.get(CONTENT_DISPOSITION).unwrap().to_str().unwrap();
        assert!(
            disposition.contains("capture-"),
            "disposition={disposition}"
        );
        assert!(!out.is_empty(), "match-all found packets in the window");
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// An explicit empty filter means "all packets in the window",
    /// identical to omitting the filter.
    #[tokio::test]
    async fn freeform_empty_filter_matches_all() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            filter: Some(String::new()),
            start: Some(FIXTURE_START.to_string()),
            duration: Some("5m".to_string()),
            ..Default::default()
        };
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!out.is_empty());
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// Standalone with no start / before / after: nothing selects a
    /// window, so the request is rejected up front.
    #[tokio::test]
    async fn standalone_missing_start_returns_400() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody::default();
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "bad-request");
        assert!(permits_free(&context));
    }

    /// Standalone with no event and no local or remote source returns
    /// 503 no-source.
    #[tokio::test]
    async fn sources_list_local_spool_and_pcap_agents() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;

        // Local spool only.
        assert_eq!(
            list_sources(&context),
            json!({ "sources": [{ "name": "(server)", "kind": "server" }] })
        );

        // A connected pcap-capable agent joins the list.
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        context
            .agents
            .register(
                crate::agent::protocol::AgentHandshake {
                    name: "remote".to_string(),
                    hostname: "remote-host".to_string(),
                    version: "test".to_string(),
                    capabilities: vec![crate::agent::protocol::CAPABILITY_PCAP.to_string()],
                },
                None,
                "127.0.0.1:0".parse().unwrap(),
                tx,
            )
            .unwrap();
        assert_eq!(
            list_sources(&context),
            json!({ "sources": [
                { "name": "(server)", "kind": "server" },
                { "name": "remote", "kind": "agent", "hostname": "remote-host" },
            ]})
        );
    }

    #[tokio::test]
    async fn standalone_without_source_returns_503() {
        let dir = tempfile::tempdir().unwrap();
        let context = context_with_event_and_optional_source(
            dir.path(),
            matching_event(),
            PcapSettings::default(),
            None,
        )
        .await;
        let body = PcapRequestBody {
            start: Some(FIXTURE_START.to_string()),
            duration: Some("5m".to_string()),
            ..Default::default()
        };
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "no-source");
    }

    /// before/after with no event to anchor the window is a bad request.
    #[tokio::test]
    async fn relative_without_event_returns_400() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            before: Some("1m".to_string()),
            ..Default::default()
        };
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "bad-request");
    }

    /// A malformed BPF filter has no server-side pre-flight — the
    /// extraction's per-file compile rejects it (an expression that
    /// compiles on no file is a format error, not a no-match) and the
    /// libpcap message surfaces through the download result. Permits
    /// are released like any other failed request.
    #[tokio::test]
    async fn bad_filter_fails_extraction_with_format_error() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            filter: Some("notavalidbpftoken".to_string()),
            start: Some(FIXTURE_START.to_string()),
            duration: Some("5m".to_string()),
            ..Default::default()
        };
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "format");
        assert!(
            json["error"]["message"].as_str().is_some(),
            "the libpcap error is surfaced"
        );
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// The native browser transfer keeps extraction faults as structured JSON;
    /// the hidden same-origin frame can surface this error instead of saving it
    /// under the capture filename.
    #[tokio::test]
    async fn native_bad_filter_returns_structured_format_error() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            filter: Some("notavalidbpftoken".to_string()),
            start: Some(FIXTURE_START.to_string()),
            duration: Some("5m".to_string()),
            ..Default::default()
        };
        let (status, headers, out) = run_native(&context, body).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
        assert!(headers.get(CONTENT_DISPOSITION).is_none());
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "format");
        wait_for("supervisor settled", || permits_free(&context)).await;
    }

    /// An unparseable start time is a bad request, not a filter error.
    #[tokio::test]
    async fn bad_start_returns_400() {
        let dir = tempfile::tempdir().unwrap();
        let context =
            context_with_event(dir.path(), matching_event(), PcapSettings::default()).await;
        let body = PcapRequestBody {
            start: Some("not-a-timestamp".to_string()),
            ..Default::default()
        };
        let (status, _headers, out) = run(&context, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["code"], "bad-request");
    }

    fn remote_audit() -> AuditContext {
        AuditContext {
            user: "tester".to_string(),
            remote: "test".to_string(),
            event_id: "1".to_string(),
            mode: "default",
            filter: "all".to_string(),
            window: "test".to_string(),
            source: "sensor-a".to_string(),
            native: false,
        }
    }

    #[test]
    fn remote_terminal_outcomes_map_to_http_responses() {
        let audit = remote_audit();

        // An empty truncation classifies as Complete and responds 200.
        let result = PcapResult {
            code: PcapResultCode::Complete,
            upload: PcapUploadStatus::None,
            message: None,
            stats: Some(crate::agent::protocol::WireStats {
                truncated: true,
                ..Default::default()
            }),
        };
        let outcome = tasks::classify_result(&result, &UploadState::Pending, true, 0);
        let response = remote_empty_response(outcome, result.stats, &audit, "capture.pcap");
        assert_eq!(response.status(), StatusCode::OK);

        let result = PcapResult {
            code: PcapResultCode::Error,
            upload: PcapUploadStatus::Failed,
            message: Some("failed".to_string()),
            stats: None,
        };
        let upload = UploadState::Failed {
            reason: "agent-error",
            bytes: 0,
        };
        let outcome = tasks::classify_result(&result, &upload, true, 0);
        let response = remote_empty_response(outcome, result.stats, &audit, "capture.pcap");
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn protocol_outcomes_map_to_bad_gateway() {
        let audit = remote_audit();
        let response = remote_empty_response(
            RemoteOutcome::Protocol {
                detail: "empty result arrived after an upload had started".to_string(),
            },
            None,
            &audit,
            "capture.pcap",
        );
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn dropping_the_task_guard_cancels_and_removes_the_job() {
        use crate::server::agents::AgentRegistry;

        let tasks = Arc::new(tasks::Registry::default());
        let _handles = tasks
            .register(
                "job-1".to_string(),
                "secret".to_string(),
                "sensor-a".to_string(),
                1,
                1024,
            )
            .unwrap();
        let agents = AgentRegistry::default();
        let (tx, mut rx) = mpsc::channel(4);
        let entry = agents
            .register(
                crate::agent::protocol::AgentHandshake {
                    name: "sensor-a".to_string(),
                    hostname: "host".to_string(),
                    version: "test".to_string(),
                    capabilities: vec![crate::agent::protocol::CAPABILITY_PCAP.to_string()],
                },
                None,
                "127.0.0.1:0".parse().unwrap(),
                tx,
            )
            .unwrap();

        let mut disarmed = RemoteTaskGuard {
            tasks: tasks.clone(),
            entry: entry.clone(),
            id: "job-1".to_string(),
            token: "wrong".to_string(),
            cancel_on_drop: true,
        };
        disarmed.disarm_cancel();
        drop(disarmed);
        // Token-bound removal: the wrong token leaves the task in place.
        assert_eq!(tasks.len(), 1);
        assert!(rx.try_recv().is_err());

        drop(RemoteTaskGuard {
            tasks: tasks.clone(),
            entry,
            id: "job-1".to_string(),
            token: "secret".to_string(),
            cancel_on_drop: true,
        });
        assert_eq!(tasks.len(), 0);
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::Cancel {
                id: "job-1".to_string(),
                token: "secret".to_string(),
            }
        );
    }
}
