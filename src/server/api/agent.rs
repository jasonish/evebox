// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! HTTP surface for the persistent EveBox agent control channel.
//!
//! The HTTP request is upgraded once to a WebSocket. After that handshake the
//! socket carries bounded JSON text frames using the `evebox-agent-v1`
//! subprotocol. The upgrade requires a valid agent key — independent of
//! `authentication.required`, which only governs browser access — unless
//! `agents.allow-unauthenticated` is set, in which case accepting a keyless
//! connection is logged prominently.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Extension, State};
use axum::extract::{DefaultBodyLimit, Path};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::{Sink, SinkExt, StreamExt};
use tokio::sync::mpsc;

use axum_extra::headers::{Authorization, HeaderMapExt, authorization::Bearer};

use crate::agent::protocol::{
    AGENT_HEADER, AgentHandshake, AgentMessage, CONTROL_MESSAGE_MAX_BYTES, SUBPROTOCOL,
    ServerMessage,
};
use crate::agent::protocol::{CAPABILITY_PCAP, PCAP_CONTENT_TYPE};
use crate::prelude::*;
use crate::server::ServerContext;
use crate::server::agents::{
    AgentConnectionId, AgentKeyIdentity, AgentRegistry, OUTBOUND_CAPACITY,
};
use crate::server::main::SessionExtractor;
use crate::server::pcap::tasks::{UploadSendError, UploadSink};
use crate::sqlite::configdb::ConfigDb;

// JSON escapes can make a maximum-sized agent name several times larger on
// the wire, so leave ample room for it plus the other handshake fields.
const HANDSHAKE_HEADER_MAX_SIZE: usize = 128 * 1024;
const PING_INTERVAL: Duration = Duration::from_secs(30);
const MAX_MISSED_PONGS: u8 = 2;
const WS_SEND_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug)]
enum WebSocketSendError<E> {
    Timeout(Duration),
    Socket(E),
}

impl<E: std::fmt::Display> std::fmt::Display for WebSocketSendError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout(timeout) => write!(
                f,
                "WebSocket write timed out after {} seconds",
                timeout.as_secs()
            ),
            Self::Socket(err) => err.fmt(f),
        }
    }
}

async fn send_websocket_message<S>(
    sink: &mut S,
    message: Message,
) -> Result<(), WebSocketSendError<S::Error>>
where
    S: Sink<Message> + Unpin,
{
    send_websocket_message_with_timeout(sink, message, WS_SEND_TIMEOUT).await
}

async fn send_websocket_message_with_timeout<S>(
    sink: &mut S,
    message: Message,
    timeout: Duration,
) -> Result<(), WebSocketSendError<S::Error>>
where
    S: Sink<Message> + Unpin,
{
    match tokio::time::timeout(timeout, sink.send(message)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(WebSocketSendError::Socket(err)),
        Err(_) => Err(WebSocketSendError::Timeout(timeout)),
    }
}

/// `GET /api/agent/ws`: establish the persistent control channel.
pub(crate) async fn websocket(
    ws: WebSocketUpgrade,
    State(context): State<Arc<ServerContext>>,
    Extension(ConnectInfo(remote)): Extension<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Response {
    let key = match authenticate_agent(&context, &headers, remote).await {
        Ok(key) => key,
        Err(response) => return *response,
    };

    if !offers_subprotocol(&headers) {
        return (
            StatusCode::BAD_REQUEST,
            format!("this endpoint requires the {SUBPROTOCOL} WebSocket subprotocol"),
        )
            .into_response();
    }

    let handshake = match parse_handshake(&headers) {
        Ok(handshake) => handshake,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };

    // A key is both credential and identity. The hostname is only the
    // fallback name for the explicitly unauthenticated lab mode.
    let name = key
        .as_ref()
        .map(|key| key.name.clone())
        .unwrap_or_else(|| handshake.hostname.clone());
    if let Some(key) = &key {
        debug!("Agent {name:?} from {remote} authenticated with its agent key");
        debug_assert_eq!(name, key.name);
    } else {
        warn!(
            "Accepting UNAUTHENTICATED agent {name:?} from {remote}: agents.allow-unauthenticated is enabled"
        );
    }

    // Control messages should stay small. Keep the limits far below
    // tungstenite's defaults so an unauthenticated peer cannot make the
    // server buffer a huge message or fragmented frame sequence.
    ws.protocols([SUBPROTOCOL])
        .max_message_size(CONTROL_MESSAGE_MAX_BYTES)
        .max_frame_size(CONTROL_MESSAGE_MAX_BYTES)
        .on_upgrade(move |socket| {
            connection(
                socket,
                context.agents.clone(),
                context.configdb.clone(),
                name,
                handshake,
                key,
                remote,
            )
        })
}

/// Resolve the agent key on a control-channel upgrade.
///
/// A presented key must be valid even under `agents.allow-unauthenticated`:
/// presented-but-wrong always fails closed. `Ok(None)` is the keyless
/// allow-unauthenticated case.
async fn authenticate_agent(
    context: &ServerContext,
    headers: &HeaderMap,
    remote: SocketAddr,
) -> Result<Option<AgentKeyIdentity>, Box<Response>> {
    let Some(bearer) = headers.typed_get::<Authorization<Bearer>>() else {
        if context.config.agents_allow_unauthenticated {
            return Ok(None);
        }
        debug!("Refusing agent connection from {remote}: no agent key presented");
        return Err(Box::new(agent_key_error(
            StatusCode::UNAUTHORIZED,
            "agent-key-required",
            "this server requires an agent key: create one with `evebox config agents add <name>` \
             and set it as server.key in agent.yaml (or EVEBOX_SERVER_KEY)",
        )));
    };
    match context.configdb.verify_agent_key(bearer.token()).await {
        Ok(Some(key)) => {
            if let Err(err) = context.configdb.touch_agent_key(key.id).await {
                warn!(
                    "Failed to update agent key last-seen for {:?}: {err}",
                    key.name
                );
            }
            Ok(Some(AgentKeyIdentity {
                id: key.id,
                name: key.name,
            }))
        }
        Ok(None) => {
            warn!("Refusing agent connection from {remote}: unknown agent key");
            Err(Box::new(agent_key_error(
                StatusCode::UNAUTHORIZED,
                "agent-key-rejected",
                "unknown agent key: issue a new one with `evebox config agents add <name>`",
            )))
        }
        Err(err) => {
            error!("Agent key verification failed: {err}");
            Err(Box::new(
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "agent key verification failed",
                )
                    .into_response(),
            ))
        }
    }
}

fn agent_key_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

/// `GET /api/agents`: the general connected-agent read model.
pub(crate) async fn get_agents(
    State(context): State<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> impl IntoResponse {
    Json(context.agents.list())
}

/// Disable the application's global body limit for the streaming PCAP upload
/// route. The job's dispatched byte limit is still enforced while streaming.
pub(crate) fn upload_body_limit() -> DefaultBodyLimit {
    DefaultBodyLimit::disable()
}

/// `POST /api/agent/pcap/{id}`: stream an agent's extracted bytes into the
/// waiting remote job.
///
/// The bearer value is the random, single-use token minted for this job. It
/// correlates this upload with its control message; it is not the deferred
/// long-lived agent authentication mechanism.
pub(crate) async fn upload_pcap(
    State(context): State<Arc<ServerContext>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if !is_pcap_content_type(&headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "pcap upload requires application/vnd.tcpdump.pcap",
        )
            .into_response();
    }
    let Some(bearer) = headers.typed_get::<Authorization<Bearer>>() else {
        return unknown_upload().into_response();
    };
    let Some(sink) = context.pcap_tasks.begin_upload(&id, bearer.token()) else {
        return unknown_upload().into_response();
    };

    let stall_timeout = context.pcap.settings.stall_timeout;

    match receive_upload(&id, sink, body, stall_timeout).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(failure) => (failure.status(), failure.message()).into_response(),
    }
}

fn is_pcap_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(PCAP_CONTENT_TYPE))
}

fn unknown_upload() -> (StatusCode, &'static str) {
    (StatusCode::GONE, "unknown or expired pcap job")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UploadFailure {
    AgentError,
    AgentStalled,
    ClientClosed,
    ClientStalled,
    TooLarge,
}

impl UploadFailure {
    fn reason(self) -> &'static str {
        match self {
            Self::AgentError => "agent-error",
            Self::AgentStalled => "agent-stalled",
            Self::ClientClosed => "client-closed",
            Self::ClientStalled => "client-stalled",
            Self::TooLarge => "too-large",
        }
    }

    fn status(self) -> StatusCode {
        match self {
            Self::AgentError => StatusCode::BAD_REQUEST,
            Self::AgentStalled | Self::ClientStalled => StatusCode::REQUEST_TIMEOUT,
            Self::ClientClosed => StatusCode::GONE,
            Self::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::AgentError => "pcap upload body failed",
            Self::AgentStalled => "pcap upload stalled",
            Self::ClientClosed => "pcap request is no longer accepting upload data",
            Self::ClientStalled => "pcap upload consumer stalled",
            Self::TooLarge => "pcap upload exceeds the job limit",
        }
    }
}

/// Pump an upload under an idle-progress deadline. Empty body frames do not
/// reset the deadline; only successfully forwarded bytes count as progress.
async fn receive_upload(
    id: &str,
    mut sink: UploadSink,
    body: Body,
    stall_timeout: Duration,
) -> Result<(), UploadFailure> {
    let mut stream = body.into_data_stream();
    let mut deadline = tokio::time::Instant::now() + stall_timeout;

    loop {
        // A stream of immediately-ready empty frames must not evade the idle
        // bound by continually winning `timeout_at` without making progress.
        if tokio::time::Instant::now() >= deadline {
            warn!("PCAP upload {id:?} stalled while reading the agent body");
            sink.fail(UploadFailure::AgentStalled.reason());
            return Err(UploadFailure::AgentStalled);
        }
        let next = match tokio::time::timeout_at(deadline, stream.next()).await {
            Ok(next) => next,
            Err(_) => {
                warn!("PCAP upload {id:?} stalled while reading the agent body");
                sink.fail(UploadFailure::AgentStalled.reason());
                return Err(UploadFailure::AgentStalled);
            }
        };

        let Some(chunk) = next else {
            sink.complete();
            return Ok(());
        };
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(err) => {
                warn!("PCAP upload {id:?} body read failed: {err}");
                sink.fail(UploadFailure::AgentError.reason());
                return Err(UploadFailure::AgentError);
            }
        };
        if chunk.is_empty() {
            continue;
        }

        match tokio::time::timeout_at(deadline, sink.send(chunk)).await {
            Ok(Ok(())) => {
                deadline = tokio::time::Instant::now() + stall_timeout;
            }
            Ok(Err(UploadSendError::TooLarge)) => {
                warn!("PCAP upload {id:?} exceeded its dispatched byte limit");
                sink.fail(UploadFailure::TooLarge.reason());
                return Err(UploadFailure::TooLarge);
            }
            Ok(Err(UploadSendError::ReceiverClosed)) => {
                debug!("PCAP upload {id:?} stopped because its consumer closed");
                sink.fail(UploadFailure::ClientClosed.reason());
                return Err(UploadFailure::ClientClosed);
            }
            Err(_) => {
                warn!("PCAP upload {id:?} stalled while forwarding bytes");
                sink.fail(UploadFailure::ClientStalled.reason());
                return Err(UploadFailure::ClientStalled);
            }
        }
    }
}

fn offers_subprotocol(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|value| value.trim() == SUBPROTOCOL)
}

fn parse_handshake(headers: &HeaderMap) -> Result<AgentHandshake, &'static str> {
    let Some(value) = headers.get(AGENT_HEADER) else {
        return Err("missing x-evebox-agent header");
    };
    if value.as_bytes().len() > HANDSHAKE_HEADER_MAX_SIZE {
        return Err("x-evebox-agent header is too large");
    }
    let value = value
        .to_str()
        .map_err(|_| "x-evebox-agent header is not valid ASCII")?;
    let handshake: AgentHandshake =
        serde_json::from_str(value).map_err(|_| "x-evebox-agent header is not valid JSON")?;
    if handshake.hostname.trim().is_empty() || handshake.version.trim().is_empty() {
        return Err("x-evebox-agent hostname and version must not be empty");
    }
    Ok(handshake)
}

async fn connection(
    socket: WebSocket,
    agents: Arc<AgentRegistry>,
    configdb: Arc<ConfigDb>,
    name: String,
    handshake: AgentHandshake,
    key: Option<AgentKeyIdentity>,
    remote: SocketAddr,
) {
    let (mut sink, mut stream) = socket.split();
    // Guarantee that hello is the first application frame. Registration
    // happens only after this send, so dispatch cannot race ahead of it.
    let hello = ServerMessage::Hello {
        server_version: crate::version::version().to_string(),
        capabilities: server_capabilities(),
    };
    let hello = serde_json::to_string(&hello).expect("server hello must serialize");
    if let Err(err) = send_websocket_message(&mut sink, Message::Text(hello.into())).await {
        warn!("Agent {name:?} from {remote} disconnected before hello: {err}");
        return;
    }

    let (outbound, mut outbound_rx) = mpsc::channel::<ServerMessage>(OUTBOUND_CAPACITY);
    // The registry has already logged the refusal; the agent sees a close
    // and stays in its reconnect loop until the operator fixes the fleet.
    let Ok(entry) = agents.register(name.clone(), handshake, key, remote, outbound) else {
        let _ = sink.close().await;
        return;
    };
    let connection_id = entry.connection_id();
    info!(
        "Agent {name:?} connected from {remote} (generation {})",
        entry.generation
    );

    let mut ping_interval = tokio::time::interval(PING_INTERVAL);
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping_interval.tick().await; // consume the interval's immediate first tick
    let mut missed_pongs: u8 = 0;
    let mut last_ping: Option<Instant> = None;

    loop {
        tokio::select! {
            _ = entry.cancelled() => {
                debug!("Agent {name:?} was asked to disconnect");
                break;
            }
            outbound = outbound_rx.recv() => {
                let Some(message) = outbound else {
                    break;
                };
                let text = match serde_json::to_string(&message) {
                    Ok(text) => text,
                    Err(err) => {
                        error!("Could not serialize a control message for agent {name:?}: {err}");
                        continue;
                    }
                };
                if text.len() > CONTROL_MESSAGE_MAX_BYTES {
                    error!(
                        "Refusing oversized control message ({} bytes) for agent {name:?}",
                        text.len()
                    );
                    break;
                }
                if let Err(err) = send_websocket_message(&mut sink, Message::Text(text.into())).await {
                    warn!("Could not send a control message to agent {name:?}: {err}");
                    break;
                }
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        entry.touch();
                        if deliver_text(&agents, &connection_id, text.as_str()) == Delivery::Fatal {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        entry.touch();
                        entry.pong_received();
                        missed_pongs = 0;
                        if let Some(sent) = last_ping.take() {
                            let elapsed = sent.elapsed().as_millis();
                            entry.set_rtt(elapsed.min(u64::MAX as u128) as u64);
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        entry.touch();
                        if let Err(err) = send_websocket_message(&mut sink, Message::Pong(payload)).await {
                            warn!("Could not pong agent {name:?}: {err}");
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(_))) => {
                        entry.touch();
                        debug!("Ignoring a binary control frame from agent {name:?}");
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(err)) => {
                        warn!("Agent {name:?} control-channel read failed: {err}");
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                if missed_pongs >= MAX_MISSED_PONGS {
                    warn!("Agent {name:?} missed {missed_pongs} pongs; disconnecting");
                    break;
                }
                missed_pongs += 1;
                last_ping = Some(Instant::now());
                if let Err(err) = send_websocket_message(&mut sink, Message::Ping(Bytes::new())).await {
                    warn!("Could not ping agent {name:?}: {err}");
                    break;
                }
            }
            // An on-demand ping for a liveness probe. It rides outside the
            // periodic schedule: no missed-pong or RTT accounting, its only
            // job is to elicit a pong for the waiting probe.
            _ = entry.ping_requested() => {
                if let Err(err) = send_websocket_message(&mut sink, Message::Ping(Bytes::new())).await {
                    warn!("Could not ping agent {name:?}: {err}");
                    break;
                }
            }
        }
    }

    // Both operations are generation-aware. Always notify the consumer so it
    // can fail pending work owned by this connection even when a replacement
    // has already claimed the same name.
    agents.remove(&connection_id);
    agents.disconnected(&connection_id);
    info!(
        "Agent {name:?} from {remote} disconnected (generation {})",
        connection_id.generation
    );

    // Stamp the disconnect on the agent's key so its stored last-seen
    // records last contact rather than connect time; the admin page
    // shows it for agents that are offline.
    if let Some(key) = &entry.key
        && let Err(err) = configdb.touch_agent_key(key.id).await
    {
        warn!(
            "Failed to update agent key last-seen for {:?}: {err}",
            key.name
        );
    }
}

fn server_capabilities() -> Vec<String> {
    vec![CAPABILITY_PCAP.to_string()]
}

#[derive(Debug, PartialEq, Eq)]
enum Delivery {
    Continue,
    Fatal,
}

fn deliver_text(agents: &AgentRegistry, connection: &AgentConnectionId, text: &str) -> Delivery {
    match serde_json::from_str::<AgentMessage>(text) {
        Ok(AgentMessage::Unknown) => {
            debug!(
                "Agent {:?} sent an unknown control-message type; ignoring it",
                connection.name
            );
            Delivery::Continue
        }
        Ok(message) => {
            agents.handle_message(connection, message);
            Delivery::Continue
        }
        Err(err) => {
            warn!(
                "Agent {:?} sent a malformed control message: {err}; disconnecting",
                connection.name
            );
            Delivery::Fatal
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use std::convert::Infallible;
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};

    use axum::http::HeaderValue;
    use serde_json::{Value, json};
    use tokio::sync::Mutex;
    use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
    use tokio_tungstenite::tungstenite::client::ClientRequestBuilder;
    use tokio_tungstenite::tungstenite::http::Uri;

    use crate::agent::channel::{self, ChannelConfig};
    use crate::agent::protocol::{AgentHandshake, PcapResultCode, PcapUploadStatus, WireStats};
    use crate::eventrepo::EventRepo;
    use crate::pcap::SpoolConfig;
    use crate::server::agents::AgentMessageHandler;
    use crate::server::main::build_axum_service;
    use crate::server::metrics::Metrics;
    use crate::server::pcap::tasks::{self, UploadState};
    use crate::server::pcap::{PcapService, PcapSettings};
    use crate::server::{ServerConfig, ServerContext};
    use crate::sqlite::connection::{ConnectionBuilder, init_event_db};
    use crate::sqlite::eventrepo::SqliteEventRepo;

    use super::*;

    struct PendingSink;

    impl Sink<Message> for PendingSink {
        type Error = Infallible;

        fn poll_ready(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Pending
        }

        fn start_send(self: Pin<&mut Self>, _item: Message) -> Result<(), Self::Error> {
            Ok(())
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn websocket_writes_are_bounded() {
        let timeout = Duration::from_millis(10);
        let result = send_websocket_message_with_timeout(
            &mut PendingSink,
            Message::Ping(Bytes::new()),
            timeout,
        )
        .await;

        assert!(matches!(
            result,
            Err(WebSocketSendError::Timeout(actual)) if actual == timeout
        ));
    }

    #[test]
    fn subprotocol_must_be_offered_as_a_complete_token() {
        let mut headers = HeaderMap::new();
        assert!(!offers_subprotocol(&headers));

        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("other-v1, evebox-agent-v10"),
        );
        assert!(!offers_subprotocol(&headers));

        headers.append(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("another-v1, evebox-agent-v1"),
        );
        assert!(offers_subprotocol(&headers));
    }

    #[test]
    fn handshake_header_is_required_and_validated() {
        let mut headers = HeaderMap::new();
        assert!(parse_handshake(&headers).is_err());

        headers.insert(AGENT_HEADER, HeaderValue::from_static("not-json"));
        assert!(parse_handshake(&headers).is_err());

        headers.insert(
            AGENT_HEADER,
            HeaderValue::from_static(
                r#"{"name":"sensor-a","hostname":"host-a","version":"0.27.0","capabilities":["pcap"],"future":true}"#,
            ),
        );
        let handshake = parse_handshake(&headers).unwrap();
        assert_eq!(handshake.hostname, "host-a");
        assert_eq!(handshake.capabilities, [CAPABILITY_PCAP]);

        // A legacy agent name is accepted as an unknown field, including
        // values the current protocol would never use as an identity.
        headers.insert(
            AGENT_HEADER,
            HeaderValue::from_static(r#"{"name":"","hostname":"h","version":"v"}"#),
        );
        assert!(parse_handshake(&headers).is_ok());

        headers.insert(
            AGENT_HEADER,
            HeaderValue::from_static(r#"{"hostname":"","version":"v"}"#),
        );
        assert!(parse_handshake(&headers).is_err());
    }

    #[test]
    fn bearer_token_is_required_but_scheme_case_is_not() {
        let mut headers = HeaderMap::new();
        assert!(headers.typed_get::<Authorization<Bearer>>().is_none());

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );
        assert!(headers.typed_get::<Authorization<Bearer>>().is_none());

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("bearer job-token"),
        );
        assert_eq!(
            headers
                .typed_get::<Authorization<Bearer>>()
                .unwrap()
                .token(),
            "job-token"
        );
    }

    #[test]
    fn upload_content_type_is_explicit_but_allows_parameters() {
        let mut headers = HeaderMap::new();
        assert!(!is_pcap_content_type(&headers));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        assert!(!is_pcap_content_type(&headers));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/vnd.tcpdump.pcap; version=2.4"),
        );
        assert!(is_pcap_content_type(&headers));
    }

    fn upload(max_bytes: u64) -> (UploadSink, tasks::Handles) {
        let tasks = tasks::Registry::default();
        let handles = tasks
            .register(
                "job-1".to_string(),
                "secret".to_string(),
                "sensor-a".to_string(),
                7,
                max_bytes,
            )
            .unwrap();
        let sink = tasks.begin_upload("job-1", "secret").unwrap();
        (sink, handles)
    }

    #[tokio::test]
    async fn upload_completes_only_after_clean_eof() {
        let (sink, mut handles) = upload(8);
        receive_upload("job-1", sink, Body::from("pcap"), Duration::from_secs(1))
            .await
            .unwrap();

        assert_eq!(
            handles.body_rx.recv().await.unwrap(),
            Bytes::from_static(b"pcap")
        );
        assert_eq!(
            *handles.upload_rx.borrow(),
            UploadState::Complete { bytes: 4 }
        );
    }

    #[tokio::test]
    async fn oversized_upload_fails_without_forwarding_the_chunk() {
        let (sink, mut handles) = upload(3);
        assert_eq!(
            receive_upload("job-1", sink, Body::from("pcap"), Duration::from_secs(1),).await,
            Err(UploadFailure::TooLarge)
        );

        assert!(matches!(
            *handles.upload_rx.borrow(),
            UploadState::Failed {
                reason: "too-large",
                bytes: 0
            }
        ));
        assert!(matches!(
            handles.body_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        ));
    }

    #[tokio::test]
    async fn upload_body_error_is_not_clean_eof() {
        let (sink, handles) = upload(8);
        let body = Body::from_stream(futures::stream::once(async {
            Err::<Bytes, std::io::Error>(std::io::Error::other("broken body"))
        }));

        assert_eq!(
            receive_upload("job-1", sink, body, Duration::from_secs(1)).await,
            Err(UploadFailure::AgentError)
        );
        assert!(matches!(
            *handles.upload_rx.borrow(),
            UploadState::Failed {
                reason: "agent-error",
                bytes: 0
            }
        ));
    }

    #[tokio::test]
    async fn idle_agent_body_is_bounded() {
        let (sink, handles) = upload(8);
        let body = Body::from_stream(futures::stream::pending::<Result<Bytes, std::io::Error>>());

        assert_eq!(
            receive_upload("job-1", sink, body, Duration::from_millis(20)).await,
            Err(UploadFailure::AgentStalled)
        );
        assert!(matches!(
            *handles.upload_rx.borrow(),
            UploadState::Failed {
                reason: "agent-stalled",
                bytes: 0
            }
        ));
    }

    #[tokio::test]
    async fn stalled_consumer_is_bounded() {
        let (sink, handles) = upload(u64::MAX);
        let body = Body::from_stream(futures::stream::unfold((), |_| async {
            Some((Ok::<Bytes, std::io::Error>(Bytes::from_static(b"x")), ()))
        }));

        assert_eq!(
            receive_upload("job-1", sink, body, Duration::from_millis(20)).await,
            Err(UploadFailure::ClientStalled)
        );
        assert!(matches!(
            *handles.upload_rx.borrow(),
            UploadState::Failed {
                reason: "client-stalled",
                bytes
            } if bytes > 0
        ));
    }

    struct RecordingHandler {
        messages: AtomicUsize,
        disconnects: AtomicUsize,
    }

    impl AgentMessageHandler for RecordingHandler {
        fn message(&self, _connection: &AgentConnectionId, _message: AgentMessage) {
            self.messages.fetch_add(1, Ordering::Relaxed);
        }

        fn disconnected(&self, _connection: &AgentConnectionId) {
            self.disconnects.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn decoded_messages_are_delivered() {
        let handler = Arc::new(RecordingHandler {
            messages: AtomicUsize::new(0),
            disconnects: AtomicUsize::new(0),
        });
        let agents = AgentRegistry::new(handler.clone());
        let connection = AgentConnectionId {
            name: "sensor-a".to_string(),
            generation: 7,
        };
        let text = serde_json::to_string(&AgentMessage::PcapResult {
            id: "job-1".to_string(),
            token: "token-1".to_string(),
            result: crate::agent::protocol::PcapResult {
                code: PcapResultCode::Complete,
                upload: PcapUploadStatus::None,
                message: None,
                stats: None,
            },
        })
        .unwrap();

        assert_eq!(
            deliver_text(&agents, &connection, &text),
            Delivery::Continue
        );
        assert_eq!(handler.messages.load(Ordering::Relaxed), 1);

        // Unknown message types are ignored without reaching the handler.
        assert_eq!(
            deliver_text(&agents, &connection, r#"{"type":"future-message"}"#),
            Delivery::Continue
        );
        assert_eq!(
            deliver_text(&agents, &connection, "not-json"),
            Delivery::Fatal
        );
        assert_eq!(handler.messages.load(Ordering::Relaxed), 1);

        agents.disconnected(&connection);
        assert_eq!(handler.disconnects.load(Ordering::Relaxed), 1);
    }

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/pcap/testdata")
            .join(name)
    }

    fn matching_event() -> Value {
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
        EventRepo::SQLite(SqliteEventRepo::new(
            writer,
            pool,
            Arc::new(Metrics::default()),
        ))
    }

    async fn serve_test_server(
        settings: PcapSettings,
    ) -> (
        std::net::SocketAddr,
        tokio::task::JoinHandle<()>,
        Arc<ServerContext>,
        tempfile::TempDir,
    ) {
        serve_test_server_with_config(settings, ServerConfig::default()).await
    }

    async fn serve_test_server_with_config(
        settings: PcapSettings,
        server_config: ServerConfig,
    ) -> (
        std::net::SocketAddr,
        tokio::task::JoinHandle<()>,
        Arc<ServerContext>,
        tempfile::TempDir,
    ) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let dir = tempfile::tempdir().unwrap();
        let datastore = build_repo(&dir.path().join("events.sqlite")).await;
        let mut importer = datastore.get_importer().unwrap();
        importer.submit(matching_event()).await.unwrap();
        importer.commit().await.unwrap();
        let configdb = crate::sqlite::configdb::open(Some(&dir.path().join("config.sqlite")))
            .await
            .unwrap();
        let mut context = ServerContext::new(
            server_config,
            Arc::new(configdb),
            datastore,
            Arc::new(Metrics::default()),
        );
        context.pcap = Arc::new(PcapService::new(settings, None));
        let context = Arc::new(context);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let service = build_axum_service(context.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, service).await.unwrap();
        });
        (address, server, context, dir)
    }

    async fn add_test_key(context: &ServerContext, name: &str) -> String {
        context.configdb.add_agent_key(name).await.unwrap().key
    }

    async fn serve_remote_test() -> (
        std::net::SocketAddr,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
        Arc<ServerContext>,
        tempfile::TempDir,
    ) {
        let (address, server, context, dir) = serve_test_server(PcapSettings::default()).await;
        let key = add_test_key(&context, "test-sensor").await;
        let agent = tokio::spawn(channel::run(ChannelConfig {
            server_url: format!("http://{address}"),
            hostname: "test-host".to_string(),
            server_key: Some(key),
            spool: SpoolConfig::new(testdata("spool"), None),
            disable_certificate_check: false,
        }));
        (address, server, agent, context, dir)
    }

    async fn wait_for_agent(client: &reqwest::Client, address: std::net::SocketAddr) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(response) = client
                .get(format!("http://{address}/api/agents"))
                .send()
                .await
                && let Ok(agents) = response.json::<Vec<Value>>().await
                && agents.iter().any(|agent| agent["name"] == "test-sensor")
            {
                return;
            }
            assert!(Instant::now() < deadline, "agent did not connect in time");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    type TestAgentSocket = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    async fn connect_test_agent(
        address: std::net::SocketAddr,
        key: Option<&str>,
    ) -> TestAgentSocket {
        connect_test_agent_with_legacy_name(address, key, None).await
    }

    async fn connect_test_agent_with_legacy_name(
        address: std::net::SocketAddr,
        key: Option<&str>,
        legacy_name: Option<&str>,
    ) -> TestAgentSocket {
        let uri: Uri = format!("ws://{address}/api/agent/ws").parse().unwrap();
        let mut handshake = serde_json::to_value(AgentHandshake {
            hostname: "test-host".to_string(),
            version: "test".to_string(),
            capabilities: vec![CAPABILITY_PCAP.to_string()],
        })
        .unwrap();
        if let Some(name) = legacy_name {
            handshake["name"] = name.into();
        }
        let handshake = serde_json::to_string(&handshake).unwrap();
        let mut request = ClientRequestBuilder::new(uri)
            .with_sub_protocol(SUBPROTOCOL)
            .with_header(AGENT_HEADER, handshake);
        if let Some(key) = key {
            request = request.with_header("authorization", format!("Bearer {key}"));
        }
        let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(
            response.headers().get("sec-websocket-protocol").unwrap(),
            SUBPROTOCOL
        );
        assert!(matches!(
            next_server_control(&mut socket).await,
            ServerMessage::Hello { .. }
        ));
        socket
    }

    async fn assert_upgrade_rejected(
        address: std::net::SocketAddr,
        context: &ServerContext,
        key: Option<&str>,
        subprotocol: Option<&str>,
        handshake: Option<&str>,
        expected: StatusCode,
    ) {
        let uri: Uri = format!("ws://{address}/api/agent/ws").parse().unwrap();
        let mut request = ClientRequestBuilder::new(uri);
        if let Some(key) = key {
            request = request.with_header("authorization", format!("Bearer {key}"));
        }
        if let Some(subprotocol) = subprotocol {
            request = request.with_sub_protocol(subprotocol);
        }
        if let Some(handshake) = handshake {
            request = request.with_header(AGENT_HEADER, handshake);
        }

        let connected_before = context.agents.connected();
        let err = tokio_tungstenite::connect_async(request)
            .await
            .expect_err("invalid agent upgrade must be rejected");
        let tokio_tungstenite::tungstenite::Error::Http(response) = err else {
            panic!("expected an HTTP upgrade rejection, got {err}");
        };
        assert_eq!(response.status().as_u16(), expected.as_u16());
        assert_eq!(context.agents.connected(), connected_before);
    }

    #[tokio::test]
    async fn invalid_agent_upgrades_are_rejected_without_registration() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let key = add_test_key(&context, "test-sensor").await;
        let key = Some(key.as_str());
        let valid_handshake = serde_json::to_string(&AgentHandshake {
            hostname: "test-host".to_string(),
            version: "test".to_string(),
            capabilities: vec![CAPABILITY_PCAP.to_string()],
        })
        .unwrap();

        let bad = StatusCode::BAD_REQUEST;
        assert_upgrade_rejected(address, &context, key, None, Some(&valid_handshake), bad).await;
        assert_upgrade_rejected(
            address,
            &context,
            key,
            Some("unsupported-v1"),
            Some(&valid_handshake),
            bad,
        )
        .await;
        assert_upgrade_rejected(address, &context, key, Some(SUBPROTOCOL), None, bad).await;
        assert_upgrade_rejected(
            address,
            &context,
            key,
            Some(SUBPROTOCOL),
            Some("not-json"),
            bad,
        )
        .await;
        assert_upgrade_rejected(address, &context, key, Some(SUBPROTOCOL), Some(""), bad).await;
        assert_upgrade_rejected(
            address,
            &context,
            key,
            Some(SUBPROTOCOL),
            Some(r#"{"name":"legacy","hostname":"","version":"test"}"#),
            bad,
        )
        .await;

        server.abort();
    }

    #[tokio::test]
    async fn agent_upgrades_require_a_valid_key() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let handshake = serde_json::to_string(&AgentHandshake {
            hostname: "test-host".to_string(),
            version: "test".to_string(),
            capabilities: vec![CAPABILITY_PCAP.to_string()],
        })
        .unwrap();

        let removed = add_test_key(&context, "removed-sensor").await;
        assert!(
            context
                .configdb
                .remove_agent_key("removed-sensor")
                .await
                .unwrap()
        );

        let unauthorized = StatusCode::UNAUTHORIZED;
        for key in [
            None,
            Some("eba_unknown"),
            Some("no-prefix"),
            Some(removed.as_str()),
        ] {
            assert_upgrade_rejected(
                address,
                &context,
                key,
                Some(SUBPROTOCOL),
                Some(&handshake),
                unauthorized,
            )
            .await;
        }

        server.abort();
    }

    #[tokio::test]
    async fn allow_unauthenticated_accepts_keyless_but_not_wrong_keys() {
        let config = ServerConfig {
            agents_allow_unauthenticated: true,
            ..ServerConfig::default()
        };
        let (address, server, context, _dir) =
            serve_test_server_with_config(PcapSettings::default(), config).await;
        let handshake = serde_json::to_string(&AgentHandshake {
            hostname: "test-host".to_string(),
            version: "test".to_string(),
            capabilities: vec![CAPABILITY_PCAP.to_string()],
        })
        .unwrap();

        // A presented key must still be valid: presented-but-wrong fails
        // closed even in the lab escape hatch.
        assert_upgrade_rejected(
            address,
            &context,
            Some("eba_wrong"),
            Some(SUBPROTOCOL),
            Some(&handshake),
            StatusCode::UNAUTHORIZED,
        )
        .await;

        // No key at all is accepted in this mode.
        let _socket = connect_test_agent(address, None).await;
        wait_until(|| context.agents.connected() == 1).await;

        server.abort();
    }

    async fn wait_until<F: Fn() -> bool>(condition: F) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !condition() {
            assert!(Instant::now() < deadline, "condition not met in time");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Agents through 0.28 sent a separately configured name. Keep accepting
    /// that field, but let the key determine the connection identity.
    #[tokio::test]
    async fn a_legacy_agent_id_is_ignored_in_favor_of_the_key_name() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let other = add_test_key(&context, "other-sensor").await;
        let _socket = connect_test_agent_with_legacy_name(
            address,
            Some(&other),
            Some("legacy-configured-id"),
        )
        .await;
        wait_until(|| context.agents.connected() == 1).await;

        assert!(context.agents.get("legacy-configured-id").is_none());
        let entry = context.agents.get("other-sensor").unwrap();
        assert_eq!(entry.key.as_ref().unwrap().name, "other-sensor");

        server.abort();
    }

    #[tokio::test]
    async fn an_agent_key_stamps_its_name_on_submitted_events() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let key = add_test_key(&context, "keyed-sensor").await;
        let mut events = context.firehose.subscribe();
        let mut event = matching_event();
        event["evebox"]["agent"]["id"] = "untrusted-agent-id".into();

        let response = reqwest::Client::new()
            .post(format!("http://{address}/api/submit"))
            .header(crate::agent::protocol::AGENT_KEY_HEADER, &key)
            .body(event.to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let submitted = events.recv().await.unwrap();
        assert_eq!(submitted["evebox"]["agent"]["id"], "keyed-sensor");

        // Without a key the stamp is untrusted and removed.
        let mut event = matching_event();
        event["evebox"]["agent"]["id"] = "untrusted-agent-id".into();
        let response = reqwest::Client::new()
            .post(format!("http://{address}/api/submit"))
            .body(event.to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let submitted = events.recv().await.unwrap();
        assert!(submitted["evebox"]["agent"]["id"].is_null());

        // Stripping must not add empty structure to events without a stamp.
        let event = matching_event();
        assert!(event.get("evebox").is_none());
        let response = reqwest::Client::new()
            .post(format!("http://{address}/api/submit"))
            .body(event.to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let submitted = events.recv().await.unwrap();
        assert!(submitted.get("evebox").is_none());

        let response = reqwest::Client::new()
            .post(format!("http://{address}/api/submit"))
            .header(crate::agent::protocol::AGENT_KEY_HEADER, "eba_unknown")
            .body(matching_event().to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        server.abort();
    }

    /// Direct configuration-database removal (the CLI path) cannot notify a
    /// running server, so it applies when an agent next connects. The admin
    /// API separately bumps a live connection after removing its key.
    #[tokio::test]
    async fn cli_style_key_removal_applies_at_connect_time() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let old_key = add_test_key(&context, "test-sensor").await;
        let first = connect_test_agent(address, Some(&old_key)).await;
        wait_until(|| context.agents.connected() == 1).await;
        let held = context.agents.get("test-sensor").unwrap().generation;

        assert!(
            context
                .configdb
                .remove_agent_key("test-sensor")
                .await
                .unwrap()
        );
        let new_key = add_test_key(&context, "test-sensor").await;

        // The replacement key authenticates, but is refused post-hello while
        // the removed key's connection still owns the name.
        let mut refused = connect_test_agent(address, Some(&new_key)).await;
        match refused.next().await {
            None | Some(Ok(TungsteniteMessage::Close(_))) | Some(Err(_)) => {}
            other => panic!("expected the refused connection to close, got {other:?}"),
        }
        assert_eq!(context.agents.connected(), 1);
        assert_eq!(context.agents.get("test-sensor").unwrap().generation, held);

        // Once that connection ends (e.g. a restart), the replacement takes
        // the name.
        drop(first);
        wait_until(|| context.agents.connected() == 0).await;
        let _replacement = connect_test_agent(address, Some(&new_key)).await;
        wait_until(|| context.agents.connected() == 1).await;

        // The removed key can no longer authenticate at all.
        let handshake = serde_json::to_string(&AgentHandshake {
            hostname: "test-host".to_string(),
            version: "test".to_string(),
            capabilities: vec![CAPABILITY_PCAP.to_string()],
        })
        .unwrap();
        assert_upgrade_rejected(
            address,
            &context,
            Some(&old_key),
            Some(SUBPROTOCOL),
            Some(&handshake),
            StatusCode::UNAUTHORIZED,
        )
        .await;

        server.abort();
    }

    /// A disconnect stamps the key's stored last-seen, so offline agents
    /// report last contact rather than connect time.
    #[tokio::test]
    async fn disconnect_updates_key_last_seen() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let key = add_test_key(&context, "test-sensor").await;
        let socket = connect_test_agent(address, Some(&key)).await;
        wait_until(|| context.agents.connected() == 1).await;

        // Clear the connect-time stamp so the disconnect's write is
        // observable (CURRENT_TIMESTAMP only has second granularity).
        sqlx::query("UPDATE agent_keys SET last_seen = NULL")
            .execute(&context.configdb.pool)
            .await
            .unwrap();

        drop(socket);
        wait_until(|| context.agents.connected() == 0).await;

        // The stamp lands after the registry removal; poll for it.
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let row = context
                .configdb
                .verify_agent_key(&key)
                .await
                .unwrap()
                .unwrap();
            if row.last_seen.is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "disconnect did not update the key's last-seen"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        server.abort();
    }

    async fn next_server_control(socket: &mut TestAgentSocket) -> ServerMessage {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(3), socket.next())
                .await
                .expect("timed out waiting for server control message")
                .expect("server closed the test WebSocket")
                .expect("test WebSocket read failed");
            match message {
                TungsteniteMessage::Text(text) => {
                    return serde_json::from_str(text.as_str()).unwrap();
                }
                TungsteniteMessage::Ping(payload) => {
                    socket
                        .send(TungsteniteMessage::Pong(payload))
                        .await
                        .unwrap();
                }
                TungsteniteMessage::Pong(_) | TungsteniteMessage::Binary(_) => {}
                TungsteniteMessage::Close(frame) => {
                    panic!("server closed test WebSocket: {frame:?}")
                }
                TungsteniteMessage::Frame(_) => {}
            }
        }
    }

    async fn next_pcap_request(socket: &mut TestAgentSocket) -> (String, String) {
        loop {
            if let ServerMessage::PcapRequest { id, token, .. } = next_server_control(socket).await
            {
                return (id, token);
            }
        }
    }

    async fn send_result(
        socket: &mut TestAgentSocket,
        id: &str,
        token: &str,
        code: PcapResultCode,
        upload: PcapUploadStatus,
        stats: Option<WireStats>,
    ) {
        let message = AgentMessage::PcapResult {
            id: id.to_string(),
            token: token.to_string(),
            result: crate::agent::protocol::PcapResult {
                code,
                upload,
                message: None,
                stats,
            },
        };
        socket
            .send(TungsteniteMessage::Text(
                serde_json::to_string(&message).unwrap().into(),
            ))
            .await
            .unwrap();
    }

    async fn wait_for_remote_cleanup(context: &ServerContext) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if context.pcap.idle() && context.pcap_tasks.len() == 0 {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("remote job and permits did not clean up");
    }

    async fn request_capture(
        client: &reqwest::Client,
        address: std::net::SocketAddr,
        request: Value,
    ) -> (HeaderMap, Bytes) {
        let response = client
            .post(format!("http://{address}/api/pcap"))
            .json(&request)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.bytes().await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "capture failed: {}",
            String::from_utf8_lossy(&bytes)
        );
        assert_eq!(headers.get("x-evebox-pcap-source").unwrap(), "test-sensor");
        assert_eq!(&bytes[..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
        (headers, bytes)
    }

    #[tokio::test]
    async fn upload_endpoint_rejects_bad_and_reused_tokens_over_http() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let _handles = context
            .pcap_tasks
            .register(
                "job-token-test".to_string(),
                "secret".to_string(),
                "test-sensor".to_string(),
                1,
                1024,
            )
            .unwrap();
        let client = reqwest::Client::new();
        let url = format!("http://{address}/api/agent/pcap/job-token-test");
        let upload = |token: &'static str| {
            client
                .post(url.clone())
                .bearer_auth(token)
                .header(reqwest::header::CONTENT_TYPE, PCAP_CONTENT_TYPE)
                .body("")
        };

        assert_eq!(upload("wrong").send().await.unwrap().status(), 410);
        assert_eq!(upload("secret").send().await.unwrap().status(), 200);
        assert_eq!(upload("secret").send().await.unwrap().status(), 410);

        context.pcap_tasks.remove("job-token-test", "secret");
        server.abort();
    }

    /// A half-open agent connection (socket open, peer no longer reading,
    /// so pings go unanswered) fails a capture request fast with a 503
    /// instead of tying up the full first-byte window.
    #[tokio::test]
    async fn unresponsive_agent_fails_fast_before_dispatch() {
        let settings = PcapSettings {
            liveness_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let (address, server, context, _dir) = serve_test_server(settings).await;
        let client = reqwest::Client::new();
        let key = add_test_key(&context, "test-sensor").await;
        // Hold the socket open but never read from it again: the probe's
        // ping is never seen, so no pong comes back.
        let _socket = connect_test_agent(address, Some(&key)).await;
        wait_for_agent(&client, address).await;

        let response = client
            .post(format!("http://{address}/api/pcap"))
            .json(&json!({ "event_id": "1" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], "agent-unresponsive");

        // The refusal happened before the job was registered; permits are
        // already back.
        wait_for_remote_cleanup(&context).await;
        server.abort();
    }

    #[tokio::test]
    async fn disconnect_before_upload_fails_promptly_and_releases_permits() {
        let settings = PcapSettings {
            request_timeout: Duration::from_secs(1),
            stall_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let (address, server, context, _dir) = serve_test_server(settings).await;
        let client = reqwest::Client::new();
        let key = add_test_key(&context, "test-sensor").await;
        let mut socket = connect_test_agent(address, Some(&key)).await;
        wait_for_agent(&client, address).await;

        let request_client = client.clone();
        let browser = tokio::spawn(async move {
            request_client
                .post(format!("http://{address}/api/pcap"))
                .json(&json!({ "event_id": "1" }))
                .send()
                .await
                .unwrap()
        });
        let _request = next_pcap_request(&mut socket).await;
        drop(socket);

        let response = tokio::time::timeout(Duration::from_secs(2), browser)
            .await
            .expect("browser request did not fail promptly")
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        wait_for_remote_cleanup(&context).await;
        server.abort();
    }

    #[tokio::test]
    async fn first_byte_timeout_sends_cancel_and_cleans_up() {
        let settings = PcapSettings {
            request_timeout: Duration::from_millis(50),
            stall_timeout: Duration::from_secs(1),
            ..Default::default()
        };
        let (address, server, context, _dir) = serve_test_server(settings).await;
        let client = reqwest::Client::new();
        let key = add_test_key(&context, "test-sensor").await;
        let mut socket = connect_test_agent(address, Some(&key)).await;
        wait_for_agent(&client, address).await;

        let request_client = client.clone();
        let browser = tokio::spawn(async move {
            request_client
                .post(format!("http://{address}/api/pcap"))
                .json(&json!({ "event_id": "1" }))
                .send()
                .await
                .unwrap()
        });
        let (id, token) = next_pcap_request(&mut socket).await;
        let response = browser.await.unwrap();
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            next_server_control(&mut socket).await,
            ServerMessage::Cancel {
                id: id.clone(),
                token: token.clone(),
            }
        );
        wait_for_remote_cleanup(&context).await;

        // The job is already gone; a late terminal result is silently
        // ignored and the connection stays usable for the next request.
        send_result(
            &mut socket,
            &id,
            &token,
            PcapResultCode::Cancelled,
            PcapUploadStatus::None,
            None,
        )
        .await;
        let followup_client = client.clone();
        let followup = tokio::spawn(async move {
            followup_client
                .post(format!("http://{address}/api/pcap"))
                .json(&json!({ "event_id": "1" }))
                .send()
                .await
                .unwrap()
        });
        let (next_id, next_token) = next_pcap_request(&mut socket).await;
        send_result(
            &mut socket,
            &next_id,
            &next_token,
            PcapResultCode::NoMatch,
            PcapUploadStatus::None,
            Some(WireStats::default()),
        )
        .await;
        assert_eq!(followup.await.unwrap().status(), StatusCode::NOT_FOUND);
        wait_for_remote_cleanup(&context).await;
        server.abort();
    }

    #[tokio::test]
    async fn missing_terminal_result_fails_the_stream_and_cleans_up_promptly() {
        let stall_timeout = Duration::from_millis(400);
        let settings = PcapSettings {
            request_timeout: Duration::from_secs(1),
            stall_timeout,
            ..Default::default()
        };
        let (address, server, context, _dir) = serve_test_server(settings).await;
        let client = reqwest::Client::new();
        let key = add_test_key(&context, "test-sensor").await;
        let mut socket = connect_test_agent(address, Some(&key)).await;
        wait_for_agent(&client, address).await;

        let request_client = client.clone();
        let browser = tokio::spawn(async move {
            request_client
                .get(format!("http://{address}/api/pcap?event_id=1"))
                .send()
                .await
                .unwrap()
        });
        let (id, token) = next_pcap_request(&mut socket).await;
        let upload = client
            .post(format!("http://{address}/api/agent/pcap/{id}"))
            .bearer_auth(&token)
            .header(reqwest::header::CONTENT_TYPE, PCAP_CONTENT_TYPE)
            .body("pcap")
            .send()
            .await
            .unwrap();
        assert_eq!(upload.status(), StatusCode::OK);

        let response = browser.await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        tokio::time::timeout(Duration::from_secs(2), response.bytes())
            .await
            .expect("browser stream did not hit the terminal-result timeout")
            .expect_err("missing terminal result must tear the browser stream");
        assert_eq!(
            next_server_control(&mut socket).await,
            ServerMessage::Cancel {
                id: id.clone(),
                token: token.clone(),
            }
        );

        // The stream supervisor already waited `stall_timeout` for the
        // missing result; cleanup after that is immediate.
        tokio::time::timeout(stall_timeout / 2, async {
            loop {
                if context.pcap.idle() && context.pcap_tasks.len() == 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("job cleanup should be immediate after the supervisor ends");

        let followup_client = client.clone();
        let followup = tokio::spawn(async move {
            followup_client
                .post(format!("http://{address}/api/pcap"))
                .json(&json!({ "event_id": "1" }))
                .send()
                .await
                .unwrap()
        });
        let (next_id, next_token) = next_pcap_request(&mut socket).await;
        send_result(
            &mut socket,
            &next_id,
            &next_token,
            PcapResultCode::NoMatch,
            PcapUploadStatus::None,
            Some(WireStats::default()),
        )
        .await;
        assert_eq!(followup.await.unwrap().status(), StatusCode::NOT_FOUND);
        wait_for_remote_cleanup(&context).await;
        server.abort();
    }

    /// A job lives no longer than the control connection it was dispatched
    /// on: replacing the channel mid-upload fails the browser request and the
    /// upload, and a fresh request on the replacement works.
    #[tokio::test]
    async fn channel_replacement_fails_the_in_flight_request() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let client = reqwest::Client::new();
        let key = add_test_key(&context, "test-sensor").await;
        let mut first_socket = connect_test_agent(address, Some(&key)).await;
        wait_for_agent(&client, address).await;

        let request_client = client.clone();
        let browser = tokio::spawn(async move {
            request_client
                .get(format!("http://{address}/api/pcap?event_id=1"))
                .send()
                .await
                .unwrap()
        });
        let (id, token) = next_pcap_request(&mut first_socket).await;

        let (body_tx, body_rx) = mpsc::channel::<Bytes>(4);
        let body_stream = futures::stream::unfold(body_rx, |mut rx| async move {
            rx.recv()
                .await
                .map(|chunk| (Ok::<Bytes, std::io::Error>(chunk), rx))
        });
        let upload_client = client.clone();
        let upload_id = id.clone();
        let upload_token = token.clone();
        let upload = tokio::spawn(async move {
            upload_client
                .post(format!("http://{address}/api/agent/pcap/{upload_id}"))
                .bearer_auth(upload_token)
                .header(reqwest::header::CONTENT_TYPE, PCAP_CONTENT_TYPE)
                .body(reqwest::Body::wrap_stream(body_stream))
                .send()
                .await
                .unwrap()
        });
        body_tx.send(Bytes::from_static(b"pcap")).await.unwrap();
        let response = browser.await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !context.pcap_tasks.is_uploading(&id) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("upload did not start");

        drop(first_socket);
        let mut replacement = connect_test_agent(address, Some(&key)).await;

        // The browser stream is torn down rather than ended cleanly.
        tokio::time::timeout(Duration::from_secs(2), response.bytes())
            .await
            .expect("browser stream did not fail after the channel was replaced")
            .expect_err("a replaced channel must fail the in-flight download");
        // The upload's consumer is gone; the next chunk surfaces the failure.
        let _ = body_tx.send(Bytes::from_static(b"tail")).await;
        drop(body_tx);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), upload)
                .await
                .expect("upload did not resolve")
                .unwrap()
                .status(),
            StatusCode::GONE
        );
        wait_for_remote_cleanup(&context).await;

        let followup_client = client.clone();
        let followup = tokio::spawn(async move {
            followup_client
                .post(format!("http://{address}/api/pcap"))
                .json(&json!({ "event_id": "1" }))
                .send()
                .await
                .unwrap()
        });
        let (next_id, next_token) = next_pcap_request(&mut replacement).await;
        send_result(
            &mut replacement,
            &next_id,
            &next_token,
            PcapResultCode::NoMatch,
            PcapUploadStatus::None,
            Some(WireStats::default()),
        )
        .await;
        assert_eq!(followup.await.unwrap().status(), StatusCode::NOT_FOUND);
        wait_for_remote_cleanup(&context).await;
        server.abort();
    }

    #[tokio::test]
    async fn browser_disconnect_reaches_agent_and_followup_request_works() {
        let (address, server, context, _dir) = serve_test_server(PcapSettings::default()).await;
        let client = reqwest::Client::new();
        let key = add_test_key(&context, "test-sensor").await;
        let mut socket = connect_test_agent(address, Some(&key)).await;
        wait_for_agent(&client, address).await;

        let request_client = client.clone();
        let browser = tokio::spawn(async move {
            request_client
                .get(format!("http://{address}/api/pcap?event_id=1"))
                .send()
                .await
                .unwrap()
        });
        let (id, token) = next_pcap_request(&mut socket).await;
        let (body_tx, body_rx) = mpsc::channel::<Bytes>(2);
        let body_stream = futures::stream::unfold(body_rx, |mut rx| async move {
            rx.recv()
                .await
                .map(|chunk| (Ok::<Bytes, std::io::Error>(chunk), rx))
        });
        let upload_client = client.clone();
        let upload_id = id.clone();
        let upload_token = token.clone();
        let upload = tokio::spawn(async move {
            upload_client
                .post(format!("http://{address}/api/agent/pcap/{upload_id}"))
                .bearer_auth(upload_token)
                .header(reqwest::header::CONTENT_TYPE, PCAP_CONTENT_TYPE)
                .body(reqwest::Body::wrap_stream(body_stream))
                .send()
                .await
        });
        body_tx.send(Bytes::from_static(b"pcap")).await.unwrap();
        let response = browser.await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        drop(response);

        assert_eq!(
            next_server_control(&mut socket).await,
            ServerMessage::Cancel {
                id: id.clone(),
                token: token.clone(),
            }
        );
        upload.abort();
        drop(body_tx);
        // The cancelled job's terminal result is accepted and ignored.
        send_result(
            &mut socket,
            &id,
            &token,
            PcapResultCode::Cancelled,
            PcapUploadStatus::Failed,
            None,
        )
        .await;
        wait_for_remote_cleanup(&context).await;

        let followup_client = client.clone();
        let followup = tokio::spawn(async move {
            followup_client
                .post(format!("http://{address}/api/pcap"))
                .json(&json!({ "event_id": "1" }))
                .send()
                .await
                .unwrap()
        });
        let (next_id, next_token) = next_pcap_request(&mut socket).await;
        send_result(
            &mut socket,
            &next_id,
            &next_token,
            PcapResultCode::NoMatch,
            PcapUploadStatus::None,
            Some(WireStats::default()),
        )
        .await;
        assert_eq!(followup.await.unwrap().status(), StatusCode::NOT_FOUND);
        wait_for_remote_cleanup(&context).await;
        server.abort();
    }

    #[tokio::test]
    async fn remote_truncation_and_native_get_validation_keep_api_parity() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (address, server, agent, context, _dir) = serve_remote_test().await;
        let client = reqwest::Client::new();
        wait_for_agent(&client, address).await;

        let response = client
            .post(format!("http://{address}/api/pcap"))
            .json(&json!({ "event_id": "1", "max_size": "1" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-evebox-pcap-truncated").unwrap(),
            "true"
        );
        assert!(response.bytes().await.unwrap().is_empty());

        let expected = std::fs::read(testdata("expected.pcap")).unwrap();
        let response = client
            .post(format!("http://{address}/api/pcap"))
            .json(&json!({
                "event_id": "1",
                "max_size": (expected.len() - 1).to_string()
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-evebox-pcap-truncated").unwrap(),
            "true"
        );
        let partial = response.bytes().await.unwrap();
        assert!(!partial.is_empty());
        assert!(partial.len() < expected.len());

        let validation = client
            .get(format!(
                "http://{address}/api/pcap/validate?event_id=1&max_size=unlimited"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(validation.status(), StatusCode::OK);
        assert_eq!(validation.json::<Value>().await.unwrap()["ok"], true);

        let unlimited = client
            .get(format!(
                "http://{address}/api/pcap?event_id=1&max_size=unlimited"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(unlimited.status(), StatusCode::OK);
        assert_eq!(unlimited.bytes().await.unwrap().as_ref(), expected);

        let finite = client
            .get(format!("http://{address}/api/pcap?event_id=1&max_size=100"))
            .send()
            .await
            .unwrap();
        assert_eq!(finite.status(), StatusCode::OK);
        let finite = finite.bytes().await.unwrap();
        assert!(!finite.is_empty());
        assert!(finite.len() <= 100);

        let buffered_unlimited = client
            .post(format!("http://{address}/api/pcap"))
            .json(&json!({ "event_id": "1", "max_size": "unlimited" }))
            .send()
            .await
            .unwrap();
        assert_eq!(buffered_unlimited.status(), StatusCode::PAYLOAD_TOO_LARGE);
        wait_for_remote_cleanup(&context).await;
        agent.abort();
        server.abort();
    }

    /// Exercise the actual TCP listener, HTTP->WebSocket upgrade, JSON
    /// control request, token-bound upload, and browser response for every
    /// filter mode accepted by the existing PCAP API.
    #[tokio::test]
    async fn remote_round_trip_supports_flow_expression_and_all_filters() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (address, server, agent, _context, _dir) = serve_remote_test().await;
        let client = reqwest::Client::new();
        wait_for_agent(&client, address).await;

        let (_, flow) = request_capture(&client, address, json!({ "event_id": "1" })).await;
        assert_eq!(
            flow.as_ref(),
            std::fs::read(testdata("expected.pcap")).unwrap()
        );

        let (_, expression) = request_capture(
            &client,
            address,
            json!({
                "event_id": "1",
                "filter": "udp and host 10.1.1.5 and host 192.0.2.10",
                "start": "2023-11-14T22:13:20+00:00",
                "duration": "5m"
            }),
        )
        .await;
        assert_eq!(expression, flow);

        let (_, all) = request_capture(
            &client,
            address,
            json!({
                "source": "test-sensor",
                "start": "2023-11-14T22:13:20+00:00",
                "duration": "5m"
            }),
        )
        .await;
        assert!(
            all.len() > flow.len(),
            "all-packet capture includes fixture noise"
        );

        agent.abort();
        server.abort();
    }
}
