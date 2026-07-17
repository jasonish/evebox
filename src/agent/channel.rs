// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Persistent agent control channel and remote PCAP worker.
//!
//! The WebSocket is a small JSON control plane. Packet bytes are uploaded on
//! a separate HTTP request so future command families can share this channel
//! without putting bulk data in WebSocket frames.

use std::collections::HashMap;
use std::io::Write;
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::sync::{Semaphore, mpsc, oneshot, watch};
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::ClientRequestBuilder;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;
use tokio_util::sync::CancellationToken;

use crate::agent::protocol::{
    AGENT_HEADER, AgentHandshake, AgentMessage, CAPABILITY_PCAP, CONTROL_MESSAGE_MAX_BYTES,
    PCAP_CONTENT_TYPE, PcapResult, PcapResultCode, PcapUploadStatus, SUBPROTOCOL, ServerMessage,
    WireLimits, WirePcapFilter, WireStats, agent_pcap_upload_path,
};
use crate::pcap::{self, FetchError, PcapRequest, PcapSource, SpoolConfig};
use crate::prelude::*;

const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const HEALTHY_CONNECTION_AGE: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const RECEIVE_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const CONTROL_SEND_TIMEOUT: Duration = Duration::from_secs(20);
const UPLOAD_STALL_TIMEOUT: Duration = Duration::from_secs(60);
const UPLOAD_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
const CHUNK_SIZE: usize = 64 * 1024;
const UPLOAD_CHANNEL_CAPACITY: usize = 8;
const RESULT_CHANNEL_CAPACITY: usize = 8;

/// Bound on jobs tracked by one connection. The server dispatches one job
/// per source at a time, so accumulation beyond a few cancelled-but-wedged
/// extractions means the peer is broken; reconnect and shed the state.
const MAX_ACTIVE_JOBS: usize = 16;

/// Inputs owned by the control task and shared across reconnects.
#[derive(Debug, Clone)]
pub(crate) struct ChannelConfig {
    pub(crate) server_url: String,
    /// Unique identifier claimed on the control channel; the server routes
    /// events whose sensor identity matches it to this agent.
    pub(crate) agent_id: String,
    pub(crate) hostname: String,
    /// Agent key (`server.key` / `EVEBOX_SERVER_KEY`) presented as a bearer
    /// token on the WebSocket upgrade.
    pub(crate) server_key: Option<String>,
    pub(crate) spool: SpoolConfig,
    pub(crate) disable_certificate_check: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionOutcome {
    Disconnected { connected_for: Duration },
    ConnectFailed,
    ServerTooOld,
    Unauthorized,
}

/// Loud-once connect diagnostics, reset by a successful connection.
#[derive(Debug, Default)]
struct ConnectWarnings {
    too_old: bool,
    unauthorized: bool,
}

fn next_backoff(backoff: Duration, connected_for: Duration) -> (Duration, Duration) {
    if connected_for >= HEALTHY_CONNECTION_AGE {
        (MIN_BACKOFF, MIN_BACKOFF)
    } else {
        (backoff, (backoff * 2).min(MAX_BACKOFF))
    }
}

struct ActiveJob {
    token: String,
    cancel: CancellationToken,
}

/// Jobs accepted on one connection. A job whose control channel goes away is
/// cancelled and its result discarded: the server has already failed the
/// waiting browser request, and the user simply retries.
type Jobs = Arc<Mutex<HashMap<String, ActiveJob>>>;

/// Run the control channel forever. Callers should spawn this independently
/// of the fail-fast EVE importer task set.
pub(crate) async fn run(config: ChannelConfig) {
    let config = Arc::new(config);
    // Serializes disk extraction across connections: a blocking producer can
    // outlive the connection which started it.
    let extraction = Arc::new(Semaphore::new(1));
    // Building the upload client is deterministic; a failure would repeat on
    // every retry, so give up on the channel rather than spin.
    let client = match crate::agent::client::build_reqwest_client(config.disable_certificate_check)
    {
        Ok(client) => client,
        Err(err) => {
            error!("agent channel: failed to build PCAP upload client: {err}; channel disabled");
            return;
        }
    };
    let mut backoff = MIN_BACKOFF;
    let mut warned = ConnectWarnings::default();

    loop {
        let outcome = connect_and_run(&config, &client, &extraction, &mut warned).await;

        let delay = match outcome {
            ConnectionOutcome::Disconnected { connected_for } => {
                let (delay, next) = next_backoff(backoff, connected_for);
                backoff = next;
                delay
            }
            ConnectionOutcome::ConnectFailed => {
                let (delay, next) = next_backoff(backoff, Duration::ZERO);
                backoff = next;
                delay
            }
            ConnectionOutcome::ServerTooOld | ConnectionOutcome::Unauthorized => MAX_BACKOFF,
        };
        tokio::time::sleep(delay + jitter(delay)).await;
    }
}

async fn connect_and_run(
    config: &Arc<ChannelConfig>,
    client: &reqwest::Client,
    extraction: &Arc<Semaphore>,
    warned: &mut ConnectWarnings,
) -> ConnectionOutcome {
    let handshake = AgentHandshake {
        name: config.agent_id.clone(),
        hostname: config.hostname.clone(),
        version: crate::version::version().to_string(),
        capabilities: vec![CAPABILITY_PCAP.to_string()],
    };
    let handshake = match serde_json::to_string(&handshake) {
        Ok(handshake) => ascii_json(&handshake),
        Err(err) => {
            error!("agent channel: failed to encode handshake: {err}");
            return ConnectionOutcome::ConnectFailed;
        }
    };
    let url = match crate::agent::tls::websocket_url(&config.server_url) {
        Ok(url) => url,
        Err(err) => {
            error!(
                "agent channel: invalid server URL {:?}: {err}",
                config.server_url
            );
            return ConnectionOutcome::ConnectFailed;
        }
    };
    let uri: Uri = match url.parse() {
        Ok(uri) => uri,
        Err(err) => {
            error!("agent channel: invalid WebSocket URL {url:?}: {err}");
            return ConnectionOutcome::ConnectFailed;
        }
    };
    let mut request = ClientRequestBuilder::new(uri)
        .with_sub_protocol(SUBPROTOCOL)
        .with_header(AGENT_HEADER, handshake);
    if let Some(key) = &config.server_key {
        request = request.with_header("authorization", format!("Bearer {key}"));
    }
    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(CONTROL_MESSAGE_MAX_BYTES))
        .max_frame_size(Some(CONTROL_MESSAGE_MAX_BYTES));
    let connector = crate::agent::tls::connector(config.disable_certificate_check);
    let connect = connect_async_tls_with_config(request, Some(ws_config), false, connector);

    match tokio::time::timeout(CONNECT_TIMEOUT, connect).await {
        Ok(Ok((mut ws, response))) => {
            let selected = response
                .headers()
                .get(SEC_WEBSOCKET_PROTOCOL)
                .and_then(|value| value.to_str().ok());
            if selected != Some(SUBPROTOCOL) {
                warn!(
                    "agent channel: server selected unsupported WebSocket subprotocol {selected:?}; expected {SUBPROTOCOL:?}"
                );
                let _ = ws.close(None).await;
                return ConnectionOutcome::ConnectFailed;
            }
            *warned = ConnectWarnings::default();
            info!(
                "agent channel: connected to {} as {:?}",
                config.server_url, config.agent_id
            );
            let connected_at = Instant::now();
            run_connection(ws, config, client, extraction).await;
            ConnectionOutcome::Disconnected {
                connected_for: connected_at.elapsed(),
            }
        }
        Ok(Err(WsError::Http(response))) if response.status() == 404 => {
            if !warned.too_old {
                warn!(
                    "agent channel: server at {} does not support {}; upgrade EveBox to enable remote PCAP",
                    config.server_url,
                    crate::agent::protocol::AGENT_WS_PATH
                );
                warned.too_old = true;
            }
            ConnectionOutcome::ServerTooOld
        }
        Ok(Err(WsError::Http(response))) if response.status() == 401 => {
            if !warned.unauthorized {
                if config.server_key.is_some() {
                    error!(
                        "agent channel: the server at {} rejected the configured agent key \
                         (unknown or removed); issue a new one with `evebox config agents add {:?}` \
                         on the server and update server.key",
                        config.server_url, config.agent_id
                    );
                } else {
                    error!(
                        "agent channel: the server at {} requires an agent key; on the server run \
                         `evebox config agents add {:?}` and set the printed key as server.key in \
                         agent.yaml (or EVEBOX_SERVER_KEY)",
                        config.server_url, config.agent_id
                    );
                }
                warned.unauthorized = true;
            }
            ConnectionOutcome::Unauthorized
        }
        Ok(Err(WsError::Http(response))) if response.status() == 403 => {
            if !warned.unauthorized {
                error!(
                    "agent channel: the server at {} says the configured agent key was issued \
                     for a different agent name; issue a key for {:?} with \
                     `evebox config agents add {:?}` or set agent-id to the key's name",
                    config.server_url, config.agent_id, config.agent_id
                );
                warned.unauthorized = true;
            }
            ConnectionOutcome::Unauthorized
        }
        Ok(Err(err)) => {
            warn!(
                "agent channel: failed to connect to {}: {err}",
                config.server_url
            );
            ConnectionOutcome::ConnectFailed
        }
        Err(_) => {
            warn!(
                "agent channel: connection to {} timed out after {CONNECT_TIMEOUT:?}",
                config.server_url
            );
            ConnectionOutcome::ConnectFailed
        }
    }
}

async fn run_connection<S>(
    ws: S,
    config: &Arc<ChannelConfig>,
    client: &reqwest::Client,
    extraction: &Arc<Semaphore>,
) where
    S: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
{
    let (mut sink, mut stream) = ws.split();
    // Jobs and their results are scoped to this connection: once it is gone
    // nobody can cancel a job or receive its result, and the server has
    // already failed the waiting browser request.
    let jobs: Jobs = Arc::new(Mutex::new(HashMap::new()));
    let (result_tx, mut result_rx) = mpsc::channel::<AgentMessage>(RESULT_CHANNEL_CAPACITY);
    let mut control = ControlState::default();
    let idle = tokio::time::sleep(RECEIVE_IDLE_TIMEOUT);
    tokio::pin!(idle);

    loop {
        tokio::select! {
            incoming = stream.next() => {
                idle.as_mut().reset(tokio::time::Instant::now() + RECEIVE_IDLE_TIMEOUT);
                match incoming {
                    None => break,
                    Some(Err(err)) => {
                        warn!("agent channel: WebSocket read failed: {err}");
                        break;
                    }
                    Some(Ok(Message::Text(text))) => {
                        match handle_message(
                            text.as_str(),
                            config,
                            client,
                            &jobs,
                            &result_tx,
                            extraction,
                            &mut control,
                        ) {
                            MessageOutcome::Continue => {}
                            MessageOutcome::Fatal => break,
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if !send_control(&mut sink, Message::Pong(payload)).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                }
            }
            _ = &mut idle => {
                warn!("agent channel: no server frame for {RECEIVE_IDLE_TIMEOUT:?}; reconnecting");
                break;
            }
            result = result_rx.recv() => {
                // The channel cannot close while this loop holds result_tx.
                let Some(message) = result else { break };
                let text = match serde_json::to_string(&message) {
                    Ok(text) => text,
                    Err(err) => {
                        error!("agent channel: failed to serialize a terminal result: {err}");
                        continue;
                    }
                };
                if !send_control(&mut sink, Message::Text(text.into())).await {
                    break;
                }
            }
        }
    }

    // Stop the extractions now: this connection owned the jobs, and nothing
    // can cancel them or consume their results once it is gone.
    for job in jobs.lock().unwrap().values() {
        job.cancel.cancel();
    }
}

/// One bounded WebSocket write; a false return ends the connection.
async fn send_control<S>(sink: &mut S, message: Message) -> bool
where
    S: Sink<Message, Error = WsError> + Unpin,
{
    match tokio::time::timeout(CONTROL_SEND_TIMEOUT, sink.send(message)).await {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            warn!("agent channel: WebSocket write failed: {err}");
            false
        }
        Err(_) => {
            warn!("agent channel: WebSocket write timed out after {CONTROL_SEND_TIMEOUT:?}");
            false
        }
    }
}

#[derive(Default)]
enum ControlState {
    #[default]
    AwaitingHello,
    Ready {
        pcap: bool,
    },
}

impl ControlState {
    fn accept(&mut self, message: &ServerMessage) -> bool {
        match (&*self, message) {
            (Self::AwaitingHello, ServerMessage::Hello { capabilities, .. }) => {
                *self = Self::Ready {
                    pcap: capabilities.iter().any(|value| value == CAPABILITY_PCAP),
                };
                true
            }
            (Self::Ready { .. }, ServerMessage::Hello { .. }) => {
                warn!("agent channel: received duplicate server hello; reconnecting");
                false
            }
            (_, ServerMessage::Unknown) => true,
            (Self::AwaitingHello, _) => {
                warn!("agent channel: received control message before server hello; reconnecting");
                false
            }
            (Self::Ready { pcap: false }, ServerMessage::PcapRequest { .. }) => {
                warn!(
                    "agent channel: server sent a pcap request without advertising the pcap capability; reconnecting"
                );
                false
            }
            (Self::Ready { .. }, _) => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageOutcome {
    Continue,
    Fatal,
}

fn decode_server_message(text: &str) -> Option<ServerMessage> {
    match serde_json::from_str(text) {
        Ok(message) => Some(message),
        Err(err) => {
            warn!("agent channel: malformed server control message: {err}; reconnecting");
            None
        }
    }
}

fn handle_message(
    text: &str,
    config: &Arc<ChannelConfig>,
    client: &reqwest::Client,
    jobs: &Jobs,
    result_tx: &mpsc::Sender<AgentMessage>,
    extraction: &Arc<Semaphore>,
    control: &mut ControlState,
) -> MessageOutcome {
    let Some(message) = decode_server_message(text) else {
        return MessageOutcome::Fatal;
    };
    if !control.accept(&message) {
        return MessageOutcome::Fatal;
    }

    match message {
        ServerMessage::Hello {
            server_version,
            capabilities,
        } => {
            debug!(
                "agent channel: server hello version={server_version} capabilities={capabilities:?}"
            );
        }
        ServerMessage::PcapRequest {
            id,
            token,
            filter,
            start_us,
            end_us,
            limits,
        } => {
            match start_job(
                config, client, jobs, result_tx, extraction, id, token, filter, start_us, end_us,
                limits,
            ) {
                StartJob::Started | StartJob::Duplicate => {}
                StartJob::Conflict => return MessageOutcome::Fatal,
                StartJob::AtCapacity => {
                    // The server dispatches one job per source at a time, so
                    // this indicates a broken peer. Drop the connection: its
                    // still-pending request then fails promptly server-side.
                    warn!(
                        "agent channel: active job capacity ({MAX_ACTIVE_JOBS}) exhausted; reconnecting"
                    );
                    return MessageOutcome::Fatal;
                }
            }
        }
        ServerMessage::Cancel { id, token } => {
            let jobs = jobs.lock().unwrap();
            if let Some(job) = jobs.get(&id) {
                if job.token == token {
                    job.cancel.cancel();
                } else {
                    warn!("agent channel: ignored cancel with wrong token for job {id}");
                }
            }
        }
        ServerMessage::Unknown => {
            debug!("agent channel: ignored unknown server message type");
        }
    }
    MessageOutcome::Continue
}

enum StartJob {
    Started,
    Duplicate,
    Conflict,
    AtCapacity,
}

#[allow(clippy::too_many_arguments)]
fn start_job(
    config: &Arc<ChannelConfig>,
    client: &reqwest::Client,
    jobs: &Jobs,
    result_tx: &mpsc::Sender<AgentMessage>,
    extraction: &Arc<Semaphore>,
    id: String,
    token: String,
    filter: WirePcapFilter,
    start_us: u64,
    end_us: u64,
    limits: WireLimits,
) -> StartJob {
    let cancel = CancellationToken::new();
    {
        let mut active = jobs.lock().unwrap();
        if let Some(existing) = active.get(&id) {
            return if existing.token == token {
                StartJob::Duplicate
            } else {
                warn!("agent channel: server reused active job id {id} with a different token");
                StartJob::Conflict
            };
        }
        if active.len() >= MAX_ACTIVE_JOBS {
            return StartJob::AtCapacity;
        }
        active.insert(
            id.clone(),
            ActiveJob {
                token: token.clone(),
                cancel: cancel.clone(),
            },
        );
    }

    let config = config.clone();
    let client = client.clone();
    let extraction = extraction.clone();
    let worker_cancel = cancel.clone();
    let worker_id = id.clone();
    let worker_token = token.clone();
    // The sender moves into the blocking closure if one is launched. If the
    // async worker fails before that point it is simply dropped. Either way,
    // the receiver resolves only when no blocking producer remains alive.
    let (producer_done_tx, producer_done_rx) = oneshot::channel();
    let worker = tokio::spawn(async move {
        run_job(
            &config,
            &client,
            &extraction,
            &worker_id,
            &worker_token,
            filter,
            start_us,
            end_us,
            limits,
            &worker_cancel,
            producer_done_tx,
        )
        .await
    });

    let jobs = jobs.clone();
    let result_tx = result_tx.clone();
    // The supervisor converts even a worker panic into a terminal result. If
    // the connection is gone by then the send fails and the result is
    // discarded: the server has already failed the browser request.
    tokio::spawn(async move {
        let result = terminal_after_worker(worker, producer_done_rx).await;
        {
            let mut active = jobs.lock().unwrap();
            if active.get(&id).is_some_and(|job| job.token == token) {
                active.remove(&id);
            }
        }
        let _ = result_tx
            .send(AgentMessage::PcapResult { id, token, result })
            .await;
    });
    StartJob::Started
}

async fn terminal_after_worker(
    worker: tokio::task::JoinHandle<PcapResult>,
    producer_done: oneshot::Receiver<()>,
) -> PcapResult {
    match worker.await {
        Ok(terminal) => terminal,
        Err(err) => {
            // A running spawn_blocking closure cannot be aborted. Its latch
            // owns the true producer lifetime, so never publish settlement
            // merely because the surrounding async worker failed.
            let _ = producer_done.await;
            PcapResult::error(format!("PCAP worker failed: {err}"))
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum UploadBodyProgress {
    Pending,
    Bytes(u64),
    Eof,
    Cancelled,
}

/// Request-body stream with observable progress and EOF. The upload response
/// timer must start only after reqwest has consumed body EOF, not merely when
/// the blocking producer has finished and left bounded chunks in `rx`.
struct UploadBodyStream {
    first: Option<Bytes>,
    rx: mpsc::Receiver<Bytes>,
    cancel: CancellationToken,
    progress: watch::Sender<UploadBodyProgress>,
    bytes: u64,
    stopped: bool,
}

impl UploadBodyStream {
    fn new(
        first: Bytes,
        rx: mpsc::Receiver<Bytes>,
        cancel: CancellationToken,
    ) -> (Self, watch::Receiver<UploadBodyProgress>) {
        let (progress, progress_rx) = watch::channel(UploadBodyProgress::Pending);
        (
            Self {
                first: Some(first),
                rx,
                cancel,
                progress,
                bytes: 0,
                stopped: false,
            },
            progress_rx,
        )
    }

    fn record(&mut self, chunk: &Bytes) {
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        self.progress
            .send_replace(UploadBodyProgress::Bytes(self.bytes));
    }
}

impl Stream for UploadBodyStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.stopped {
            return std::task::Poll::Ready(None);
        }
        if this.cancel.is_cancelled() {
            this.stopped = true;
            this.progress.send_replace(UploadBodyProgress::Cancelled);
            return std::task::Poll::Ready(Some(Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "PCAP upload cancelled",
            ))));
        }
        if let Some(first) = this.first.take() {
            this.record(&first);
            return std::task::Poll::Ready(Some(Ok(first)));
        }
        match this.rx.poll_recv(cx) {
            std::task::Poll::Ready(Some(chunk)) => {
                this.record(&chunk);
                std::task::Poll::Ready(Some(Ok(chunk)))
            }
            std::task::Poll::Ready(None) => {
                this.stopped = true;
                this.progress.send_replace(UploadBodyProgress::Eof);
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_job(
    config: &ChannelConfig,
    client: &reqwest::Client,
    extraction: &Arc<Semaphore>,
    id: &str,
    token: &str,
    filter: WirePcapFilter,
    start_us: u64,
    end_us: u64,
    limits: WireLimits,
    cancel: &CancellationToken,
    producer_done: oneshot::Sender<()>,
) -> PcapResult {
    if start_us > end_us {
        return PcapResult::error("PCAP request start is after end".to_string());
    }

    let permit = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            return PcapResult::cancelled(PcapUploadStatus::None, None);
        }
        permit = extraction.clone().acquire_owned() => {
            match permit {
                Ok(permit) => permit,
                Err(_) => return PcapResult::error("PCAP extraction worker stopped".to_string()),
            }
        }
    };
    if cancel.is_cancelled() {
        return PcapResult::cancelled(PcapUploadStatus::None, None);
    }

    let request = PcapRequest {
        filter: filter.into_pcap_filter(),
        start: Some(start_us),
        end: Some(end_us),
        limits: limits.into(),
    };
    let source = PcapSource::Spool(config.spool.clone());
    let (tx, mut rx) = mpsc::channel::<Bytes>(UPLOAD_CHANNEL_CAPACITY);
    // Server cancellation propagates from the parent token. Local transport
    // failure cancels only this child, so the terminal code remains `error`
    // rather than being misreported as a server-requested cancellation.
    let fetch_cancel = cancel.child_token();
    let fetch_control = fetch_cancel.clone();
    let mut fetch = tokio::task::spawn_blocking(move || {
        // Dropped on every closure exit, including a panic. The job
        // supervisor uses this rather than the abortable async worker as the
        // authoritative producer-lifetime signal.
        let _producer_done = producer_done;
        let _permit = permit;
        let mut writer = ChannelWriter {
            tx,
            buffer: Vec::with_capacity(CHUNK_SIZE),
            cancel: fetch_cancel.clone(),
            first_sent: false,
        };
        let result = pcap::fetch(&source, &request, &mut writer, &fetch_cancel);
        match result {
            Ok(stats) => match writer.flush() {
                Ok(()) => Ok(stats),
                Err(err) => Err(FetchError::Io(err)),
            },
            Err(err) => Err(err),
        }
    });

    let first = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            fetch_control.cancel();
            return finish_cancelled_fetch(
                &mut fetch,
                None,
                cancel,
                PcapUploadStatus::None,
            ).await;
        }
        first = rx.recv() => first,
    };
    let Some(first) = first else {
        return terminal_from_fetch(fetch.await, cancel, PcapUploadStatus::None, None);
    };

    let url = pcap_upload_url(&config.server_url, id);
    let (body_stream, mut body_progress) = UploadBodyStream::new(first, rx, fetch_control.clone());
    let body = reqwest::Body::wrap_stream(body_stream);
    let upload_client = client.clone();
    let upload_token = token.to_string();
    let mut upload = tokio::spawn(async move {
        upload_client
            .post(url)
            .bearer_auth(upload_token)
            .header(reqwest::header::CONTENT_TYPE, PCAP_CONTENT_TYPE)
            .body(body)
            .send()
            .await
    });

    let mut fetch_result = None;
    let mut upload_result = None;
    let mut body_eof = false;
    let mut progress_open = true;
    let mut upload_deadline = tokio::time::Instant::now() + UPLOAD_STALL_TIMEOUT;

    while fetch_result.is_none() || upload_result.is_none() {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                // Do not leave reqwest draining the bounded body tail after a
                // browser/server cancel. Aborting the request closes the HTTP
                // body immediately; the child token stops the disk producer.
                fetch_control.cancel();
                upload.abort();
                return finish_cancelled_fetch(
                    &mut fetch,
                    fetch_result,
                    cancel,
                    PcapUploadStatus::Failed,
                ).await;
            }
            fetched = &mut fetch, if fetch_result.is_none() => {
                fetch_result = Some(fetched);
            }
            uploaded = &mut upload, if upload_result.is_none() => {
                if upload_succeeded(&uploaded) {
                    upload_result = Some(uploaded);
                } else {
                    fetch_control.cancel();
                    let (_, upload_error) = classify_upload(uploaded);
                    return finish_failed_upload(
                        &mut fetch,
                        fetch_result,
                        cancel,
                        upload_error.unwrap_or_else(|| "PCAP upload failed".to_string()),
                    ).await;
                }
            }
            changed = body_progress.changed(), if upload_result.is_none() && progress_open => {
                if changed.is_err() {
                    progress_open = false;
                } else {
                    match *body_progress.borrow_and_update() {
                        UploadBodyProgress::Pending => {}
                        UploadBodyProgress::Bytes(bytes) => {
                            trace!("agent channel: PCAP upload job {id} handed off {bytes} bytes");
                            if !body_eof {
                                upload_deadline =
                                    tokio::time::Instant::now() + UPLOAD_STALL_TIMEOUT;
                            }
                        }
                        UploadBodyProgress::Eof => {
                            body_eof = true;
                            upload_deadline =
                                tokio::time::Instant::now() + UPLOAD_RESPONSE_TIMEOUT;
                        }
                        UploadBodyProgress::Cancelled => {}
                    }
                }
            }
            _ = tokio::time::sleep_until(upload_deadline), if upload_result.is_none() => {
                let message = if body_eof {
                    format!(
                        "PCAP upload response timed out after body EOF ({UPLOAD_RESPONSE_TIMEOUT:?})"
                    )
                } else {
                    format!("PCAP upload made no progress for {UPLOAD_STALL_TIMEOUT:?}")
                };
                upload.abort();
                fetch_control.cancel();
                return finish_failed_upload(
                    &mut fetch,
                    fetch_result,
                    cancel,
                    message,
                ).await;
            }
        }
    }

    let (upload_status, upload_error) = classify_upload(upload_result.unwrap());
    terminal_from_fetch(fetch_result.unwrap(), cancel, upload_status, upload_error)
}

type FetchJoinResult = Result<Result<pcap::FetchStats, FetchError>, tokio::task::JoinError>;
type UploadJoinResult = Result<Result<reqwest::Response, reqwest::Error>, tokio::task::JoinError>;

async fn finish_cancelled_fetch(
    fetch: &mut tokio::task::JoinHandle<Result<pcap::FetchStats, FetchError>>,
    fetched: Option<FetchJoinResult>,
    cancel: &CancellationToken,
    upload: PcapUploadStatus,
) -> PcapResult {
    finish_cancelled_fetch_with_timeout(fetch, fetched, cancel, upload, UPLOAD_STALL_TIMEOUT).await
}

async fn finish_cancelled_fetch_with_timeout(
    fetch: &mut tokio::task::JoinHandle<Result<pcap::FetchStats, FetchError>>,
    fetched: Option<FetchJoinResult>,
    cancel: &CancellationToken,
    upload: PcapUploadStatus,
    stop_timeout: Duration,
) -> PcapResult {
    let fetched = await_stopped_fetch(fetch, fetched, stop_timeout).await;
    terminal_from_fetch(fetched, cancel, upload, None)
}

async fn finish_failed_upload(
    fetch: &mut tokio::task::JoinHandle<Result<pcap::FetchStats, FetchError>>,
    fetched: Option<FetchJoinResult>,
    cancel: &CancellationToken,
    message: String,
) -> PcapResult {
    let fetched = await_stopped_fetch(fetch, fetched, UPLOAD_STALL_TIMEOUT).await;
    terminal_from_fetch(fetched, cancel, PcapUploadStatus::Failed, Some(message))
}

/// Wait for the blocking producer itself, not merely its async join timeout.
///
/// `JoinHandle::abort` cannot stop a `spawn_blocking` closure after it starts.
/// Publishing a terminal result at that point would falsely tell the server
/// that the source producer and its extraction permit were gone. The server
/// has its own bounded settlement fallback, so it may release browser-facing
/// resources while this agent-side job remains active until libpcap returns.
async fn await_stopped_fetch(
    fetch: &mut tokio::task::JoinHandle<Result<pcap::FetchStats, FetchError>>,
    fetched: Option<FetchJoinResult>,
    stop_timeout: Duration,
) -> FetchJoinResult {
    if let Some(fetched) = fetched {
        return fetched;
    }
    match tokio::time::timeout(stop_timeout, &mut *fetch).await {
        Ok(fetched) => fetched,
        Err(_) => {
            warn!(
                "agent channel: PCAP blocking producer did not stop within {stop_timeout:?}; retaining the active job until it exits"
            );
            (&mut *fetch).await
        }
    }
}

fn upload_succeeded(result: &UploadJoinResult) -> bool {
    matches!(result, Ok(Ok(response)) if response.status().is_success())
}

fn classify_upload(result: UploadJoinResult) -> (PcapUploadStatus, Option<String>) {
    match result {
        Ok(Ok(response)) if response.status().is_success() => (PcapUploadStatus::Complete, None),
        Ok(Ok(response)) => (
            PcapUploadStatus::Failed,
            Some(format!("PCAP upload rejected with {}", response.status())),
        ),
        Ok(Err(err)) => (
            PcapUploadStatus::Failed,
            Some(format!("PCAP upload failed: {err}")),
        ),
        Err(err) => (
            PcapUploadStatus::Failed,
            Some(format!("PCAP upload task failed: {err}")),
        ),
    }
}

fn terminal_from_fetch(
    result: FetchJoinResult,
    cancel: &CancellationToken,
    upload: PcapUploadStatus,
    upload_error: Option<String>,
) -> PcapResult {
    let stats = match &result {
        Ok(Ok(stats)) | Ok(Err(FetchError::NoMatch(stats))) => Some(WireStats::from(stats)),
        Ok(Err(FetchError::NoCandidateFiles)) => Some(WireStats::default()),
        _ => None,
    };
    if cancel.is_cancelled() {
        return PcapResult::cancelled(upload, stats);
    }
    if let Some(message) = upload_error {
        return PcapResult {
            code: PcapResultCode::Error,
            upload,
            message: Some(message),
            stats,
        };
    }

    match result {
        Ok(Ok(stats)) => PcapResult {
            code: PcapResultCode::Complete,
            upload,
            message: None,
            stats: Some(WireStats::from(&stats)),
        },
        Ok(Err(FetchError::NoCandidateFiles)) => PcapResult {
            code: PcapResultCode::NoCandidateFiles,
            upload,
            message: None,
            stats: Some(WireStats::default()),
        },
        Ok(Err(FetchError::NoMatch(stats))) => PcapResult {
            code: PcapResultCode::NoMatch,
            upload,
            message: None,
            stats: Some(WireStats::from(&stats)),
        },
        Ok(Err(FetchError::Format(message))) => PcapResult {
            code: PcapResultCode::Error,
            upload,
            message: Some(message),
            stats: None,
        },
        Ok(Err(FetchError::Io(err))) => PcapResult {
            code: PcapResultCode::Error,
            upload,
            message: Some(err.to_string()),
            stats: None,
        },
        Err(err) => PcapResult {
            code: PcapResultCode::Error,
            upload,
            message: Some(format!("PCAP extraction task failed: {err}")),
            stats: None,
        },
    }
}

/// A bounded, cancellation-aware bridge from blocking `pcap::fetch` writes
/// to the async HTTP request body.
struct ChannelWriter {
    tx: mpsc::Sender<Bytes>,
    buffer: Vec<u8>,
    cancel: CancellationToken,
    first_sent: bool,
}

impl ChannelWriter {
    fn push(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let mut chunk = Bytes::from(std::mem::take(&mut self.buffer));
        let deadline = Instant::now() + UPLOAD_STALL_TIMEOUT;
        loop {
            match self.tx.try_send(chunk) {
                Ok(()) => {
                    self.first_sent = true;
                    return Ok(());
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "PCAP upload receiver closed",
                    ));
                }
                Err(mpsc::error::TrySendError::Full(returned)) => {
                    if self.cancel.is_cancelled() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Interrupted,
                            "PCAP request cancelled",
                        ));
                    }
                    if Instant::now() >= deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "PCAP upload stalled",
                        ));
                    }
                    chunk = returned;
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut remaining = data;
        while !remaining.is_empty() {
            let available = CHUNK_SIZE - self.buffer.len();
            let take = available.min(remaining.len());
            self.buffer.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];

            // The extractor emits nothing until its first match. Forward that
            // first write immediately so server first-byte timeouts measure
            // time to first match, not time to fill a 64 KiB transport chunk.
            // Later writes are split, not merely flushed after crossing the
            // threshold, so every queued chunk stays within the advertised
            // 64 KiB backpressure bound even for oversized capture records.
            if !self.first_sent || self.buffer.len() == CHUNK_SIZE {
                self.push()?;
            }
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.push()
    }
}

fn pcap_upload_url(server_url: &str, id: &str) -> String {
    format!("{server_url}{}", agent_pcap_upload_path(id))
}

/// JSON in an HTTP header must be ASCII. Escaping preserves the original
/// Unicode when the server deserializes the header value.
fn ascii_json(json: &str) -> String {
    use std::fmt::Write;

    let mut output = String::with_capacity(json.len());
    let mut encoded = [0_u16; 2];
    for character in json.chars() {
        if character.is_ascii() {
            output.push(character);
        } else {
            for unit in character.encode_utf16(&mut encoded) {
                let _ = write!(output, "\\u{unit:04x}");
            }
        }
    }
    output
}

fn jitter(delay: Duration) -> Duration {
    use rand::Rng;

    let max_millis = (delay.as_millis() / 2) as u64;
    if max_millis == 0 {
        Duration::ZERO
    } else {
        Duration::from_millis(rand::rng().random_range(0..=max_millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::protocol::WireEndpoint;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::routing::post;
    use tokio::sync::oneshot;

    fn hello(capabilities: &[&str]) -> ServerMessage {
        ServerMessage::Hello {
            server_version: "test".to_string(),
            capabilities: capabilities
                .iter()
                .map(|capability| (*capability).to_string())
                .collect(),
        }
    }

    fn pcap_request() -> ServerMessage {
        ServerMessage::PcapRequest {
            id: "job".to_string(),
            token: "token".to_string(),
            filter: WirePcapFilter::All,
            start_us: 1,
            end_us: 2,
            limits: WireLimits {
                max_bytes: 1024,
                scan_timeout_ms: 1000,
            },
        }
    }

    #[test]
    fn request_before_hello_is_connection_fatal() {
        assert!(!ControlState::default().accept(&pcap_request()));
    }

    #[test]
    fn duplicate_hello_is_connection_fatal() {
        let mut state = ControlState::default();
        assert!(state.accept(&hello(&[CAPABILITY_PCAP])));
        assert!(!state.accept(&hello(&[CAPABILITY_PCAP])));
    }

    #[test]
    fn request_without_negotiated_pcap_capability_is_connection_fatal() {
        let mut state = ControlState::default();
        assert!(state.accept(&hello(&[])));
        assert!(!state.accept(&pcap_request()));
    }

    #[test]
    fn hello_with_pcap_capability_allows_requests() {
        let mut state = ControlState::default();
        assert!(state.accept(&hello(&[CAPABILITY_PCAP])));
        assert!(state.accept(&pcap_request()));
    }

    #[test]
    fn malformed_control_json_is_fatal_but_unknown_types_are_tolerated() {
        assert_eq!(decode_server_message("not-json"), None);
        assert_eq!(
            decode_server_message(r#"{"type":"future-message"}"#),
            Some(ServerMessage::Unknown)
        );
    }

    #[test]
    fn backoff_only_resets_after_a_healthy_connection() {
        assert_eq!(
            next_backoff(MAX_BACKOFF, HEALTHY_CONNECTION_AGE),
            (MIN_BACKOFF, MIN_BACKOFF)
        );
        assert_eq!(
            next_backoff(MIN_BACKOFF, Duration::from_secs(2)),
            (MIN_BACKOFF, MIN_BACKOFF * 2)
        );
        assert_eq!(
            next_backoff(MAX_BACKOFF, Duration::ZERO),
            (MAX_BACKOFF, MAX_BACKOFF)
        );
    }

    #[test]
    fn upload_endpoint_uses_the_normalized_base_path() {
        assert_eq!(
            pcap_upload_url("https://evebox.test/base", "job-1"),
            "https://evebox.test/base/api/agent/pcap/job-1"
        );
    }

    #[test]
    fn unicode_handshake_json_is_ascii_and_round_trips() {
        let handshake = AgentHandshake {
            name: "föö-sensor".to_string(),
            hostname: "höst-🦀".to_string(),
            version: "1".to_string(),
            capabilities: vec![CAPABILITY_PCAP.to_string()],
        };
        let encoded = ascii_json(&serde_json::to_string(&handshake).unwrap());
        assert!(encoded.is_ascii());
        assert_eq!(
            serde_json::from_str::<AgentHandshake>(&encoded).unwrap(),
            handshake
        );
    }

    #[test]
    fn all_wire_filter_maps_to_no_engine_filter() {
        assert!(WirePcapFilter::All.into_pcap_filter().is_none());
    }

    #[test]
    fn expression_wire_filter_maps_to_expression() {
        let filter = WirePcapFilter::Expression {
            expression: "tcp port 443".to_string(),
        }
        .into_pcap_filter();
        assert!(matches!(
            filter,
            Some(crate::pcap::PcapFilter::Expression(expression))
                if expression == "tcp port 443"
        ));
    }

    #[test]
    fn flow_wire_filter_maps_to_flow() {
        let filter = WirePcapFilter::Flow {
            proto: 6,
            a: WireEndpoint {
                ip: "192.0.2.1".parse().unwrap(),
                port: Some(12345),
            },
            b: WireEndpoint {
                ip: "198.51.100.2".parse().unwrap(),
                port: Some(443),
            },
        }
        .into_pcap_filter();
        match filter {
            Some(crate::pcap::PcapFilter::Flow(flow)) => {
                assert_eq!(flow.proto, 6);
                assert_eq!(flow.a.1, Some(12345));
                assert_eq!(flow.b.1, Some(443));
            }
            other => panic!("expected flow filter, got {other:?}"),
        }
    }

    #[test]
    fn empty_fetch_outcomes_keep_local_api_semantics() {
        let cancel = CancellationToken::new();

        let terminal = terminal_from_fetch(
            Ok(Err(FetchError::NoCandidateFiles)),
            &cancel,
            PcapUploadStatus::None,
            None,
        );
        assert_eq!(terminal.code, PcapResultCode::NoCandidateFiles);
        assert_eq!(terminal.upload, PcapUploadStatus::None);
        assert_eq!(terminal.stats, Some(WireStats::default()));

        let no_match = pcap::FetchStats {
            files_scanned: 3,
            files_vanished: 1,
            ..Default::default()
        };
        let terminal = terminal_from_fetch(
            Ok(Err(FetchError::NoMatch(no_match))),
            &cancel,
            PcapUploadStatus::None,
            None,
        );
        assert_eq!(terminal.code, PcapResultCode::NoMatch);
        assert_eq!(terminal.stats.unwrap().files_scanned, 3);

        // A byte limit smaller than the first packet is an empty, truncated
        // success, not a no-match result. The server uses this terminal shape
        // to preserve the buffered POST truncation header.
        let truncated = pcap::FetchStats {
            truncated: true,
            files_scanned: 1,
            ..Default::default()
        };
        let terminal =
            terminal_from_fetch(Ok(Ok(truncated)), &cancel, PcapUploadStatus::None, None);
        assert_eq!(terminal.code, PcapResultCode::Complete);
        assert_eq!(terminal.upload, PcapUploadStatus::None);
        assert!(terminal.stats.unwrap().truncated);
    }

    #[test]
    fn upload_failure_and_server_cancel_have_distinct_terminal_codes() {
        let stats = || pcap::FetchStats {
            packets: 1,
            bytes: 64,
            files_scanned: 1,
            ..Default::default()
        };
        let cancel = CancellationToken::new();
        let terminal = terminal_from_fetch(
            Ok(Ok(stats())),
            &cancel,
            PcapUploadStatus::Failed,
            Some("upload rejected".to_string()),
        );
        assert_eq!(terminal.code, PcapResultCode::Error);
        assert_eq!(terminal.upload, PcapUploadStatus::Failed);
        assert_eq!(terminal.message.as_deref(), Some("upload rejected"));

        cancel.cancel();
        let terminal = terminal_from_fetch(
            Ok(Ok(stats())),
            &cancel,
            PcapUploadStatus::Failed,
            Some("upload rejected".to_string()),
        );
        assert_eq!(terminal.code, PcapResultCode::Cancelled);
        assert_eq!(terminal.upload, PcapUploadStatus::Failed);
        assert!(terminal.message.is_none());
    }

    #[tokio::test]
    async fn failed_async_worker_waits_for_its_blocking_producer() {
        let (producer_done_tx, producer_done_rx) = oneshot::channel();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = tokio::spawn(async move {
            let _producer = tokio::task::spawn_blocking(move || {
                let _producer_done = producer_done_tx;
                let _ = started_tx.send(());
                release_rx.recv().unwrap();
            });
            panic!("simulated async worker failure after producer launch");
        });
        started_rx.await.unwrap();

        let mut supervised = tokio::spawn(terminal_after_worker(worker, producer_done_rx));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut supervised)
                .await
                .is_err(),
            "worker failure must not publish a terminal result while its producer is alive"
        );
        release_tx.send(()).unwrap();
        let terminal = supervised.await.unwrap();
        assert_eq!(terminal.code, PcapResultCode::Error);
    }

    #[tokio::test]
    async fn cancelled_blocking_fetch_does_not_publish_terminal_before_exit() {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let fetch = tokio::task::spawn_blocking(move || {
            let _ = started_tx.send(());
            release_rx.recv().unwrap();
            Ok(pcap::FetchStats::default())
        });
        started_rx.await.unwrap();

        let cancel = CancellationToken::new();
        cancel.cancel();
        let finish_cancel = cancel.clone();
        let terminal = tokio::spawn(async move {
            let mut fetch = fetch;
            finish_cancelled_fetch_with_timeout(
                &mut fetch,
                None,
                &finish_cancel,
                PcapUploadStatus::None,
                Duration::from_millis(10),
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(
            !terminal.is_finished(),
            "a running spawn_blocking producer must not be reported terminal"
        );
        release_tx.send(()).unwrap();
        let terminal = terminal.await.unwrap();
        assert_eq!(terminal.code, PcapResultCode::Cancelled);
    }

    #[tokio::test]
    async fn upload_body_reports_progress_eof_and_cancellation() {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        let cancel = CancellationToken::new();
        let (mut body, mut progress) =
            UploadBodyStream::new(Bytes::from_static(b"pcap"), rx, cancel);
        assert_eq!(body.next().await.unwrap().unwrap().as_ref(), b"pcap");
        progress.changed().await.unwrap();
        assert!(matches!(
            *progress.borrow_and_update(),
            UploadBodyProgress::Bytes(4)
        ));
        assert!(body.next().await.is_none());
        progress.changed().await.unwrap();
        assert!(matches!(
            *progress.borrow_and_update(),
            UploadBodyProgress::Eof
        ));

        let (_tx, rx) = mpsc::channel(1);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (mut body, progress) =
            UploadBodyStream::new(Bytes::from_static(b"never-sent"), rx, cancel);
        assert_eq!(
            body.next().await.unwrap().unwrap_err().kind(),
            std::io::ErrorKind::Interrupted
        );
        assert!(matches!(*progress.borrow(), UploadBodyProgress::Cancelled));
    }

    type UploadCapture = Arc<Mutex<Option<oneshot::Sender<(HeaderMap, Bytes)>>>>;

    async fn capture_upload(
        State(capture): State<UploadCapture>,
        headers: HeaderMap,
        body: Body,
    ) -> StatusCode {
        let body = to_bytes(body, 1024 * 1024).await.unwrap();
        if let Some(tx) = capture.lock().unwrap().take() {
            let _ = tx.send((headers, body));
        }
        StatusCode::OK
    }

    type UploadStarted = Arc<Mutex<Option<oneshot::Sender<()>>>>;

    async fn stall_upload(State(started): State<UploadStarted>, _body: Body) -> StatusCode {
        if let Some(tx) = started.lock().unwrap().take() {
            let _ = tx.send(());
        }
        std::future::pending().await
    }

    async fn reject_upload(_body: Body) -> StatusCode {
        StatusCode::BAD_GATEWAY
    }

    #[tokio::test]
    async fn worker_extracts_and_uploads_a_complete_pcap() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (captured_tx, captured_rx) = oneshot::channel();
        let capture: UploadCapture = Arc::new(Mutex::new(Some(captured_tx)));
        let app = Router::new()
            .route("/api/agent/pcap/job-1", post(capture_upload))
            .with_state(capture);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let directory = tempfile::tempdir().unwrap();
        crate::pcap::testutil::write_pcap_file(
            &directory.path().join("log.pcap.1700000000"),
            &[(1_700_000_000, 53)],
        );
        let config = ChannelConfig {
            server_url: format!("http://{address}"),
            agent_id: "sensor".to_string(),
            hostname: "host".to_string(),
            server_key: None,
            spool: SpoolConfig::new(directory.path(), None),
            disable_certificate_check: false,
        };
        let client = crate::agent::client::build_reqwest_client(false).unwrap();
        let cancel = CancellationToken::new();
        let terminal = run_job(
            &config,
            &client,
            &Arc::new(Semaphore::new(1)),
            "job-1",
            "job-token",
            WirePcapFilter::All,
            1_699_999_999_000_000,
            1_700_000_001_000_000,
            WireLimits {
                max_bytes: 8_000_000,
                scan_timeout_ms: 60_000,
            },
            &cancel,
            oneshot::channel().0,
        )
        .await;

        assert_eq!(terminal.code, PcapResultCode::Complete);
        assert_eq!(terminal.upload, PcapUploadStatus::Complete);
        let stats = terminal.stats.unwrap();
        assert_eq!(stats.packets, 1);
        let (headers, body) = captured_rx.await.unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer job-token"
        );
        assert_eq!(
            headers.get(reqwest::header::CONTENT_TYPE).unwrap(),
            PCAP_CONTENT_TYPE
        );
        assert_eq!(u64::try_from(body.len()).unwrap(), stats.bytes);
        assert_eq!(&body[..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
        server.abort();
    }

    #[tokio::test]
    async fn server_cancel_aborts_an_active_http_upload_promptly() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (started_tx, started_rx) = oneshot::channel();
        let started: UploadStarted = Arc::new(Mutex::new(Some(started_tx)));
        let app = Router::new()
            .route("/api/agent/pcap/job-cancel", post(stall_upload))
            .with_state(started);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let directory = tempfile::tempdir().unwrap();
        crate::pcap::testutil::write_pcap_file(
            &directory.path().join("log.pcap.1700000000"),
            &[(1_700_000_000, 53)],
        );
        let config = ChannelConfig {
            server_url: format!("http://{address}"),
            agent_id: "sensor".to_string(),
            hostname: "host".to_string(),
            server_key: None,
            spool: SpoolConfig::new(directory.path(), None),
            disable_certificate_check: false,
        };
        let client = crate::agent::client::build_reqwest_client(false).unwrap();
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let job = tokio::spawn(async move {
            run_job(
                &config,
                &client,
                &Arc::new(Semaphore::new(1)),
                "job-cancel",
                "job-token",
                WirePcapFilter::All,
                1_699_999_999_000_000,
                1_700_000_001_000_000,
                WireLimits {
                    max_bytes: 8_000_000,
                    scan_timeout_ms: 60_000,
                },
                &worker_cancel,
                oneshot::channel().0,
            )
            .await
        });

        tokio::time::timeout(Duration::from_secs(2), started_rx)
            .await
            .expect("upload request should start")
            .unwrap();
        cancel.cancel();
        let terminal = tokio::time::timeout(Duration::from_secs(2), job)
            .await
            .expect("cancelled upload should finish promptly")
            .unwrap();
        assert_eq!(terminal.code, PcapResultCode::Cancelled);
        assert_eq!(terminal.upload, PcapUploadStatus::Failed);
        server.abort();
    }

    #[tokio::test]
    async fn rejected_upload_returns_a_terminal_error_promptly() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let app = Router::new().route("/api/agent/pcap/job-rejected", post(reject_upload));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let directory = tempfile::tempdir().unwrap();
        crate::pcap::testutil::write_pcap_file(
            &directory.path().join("log.pcap.1700000000"),
            &[(1_700_000_000, 53)],
        );
        let config = ChannelConfig {
            server_url: format!("http://{address}"),
            agent_id: "sensor".to_string(),
            hostname: "host".to_string(),
            server_key: None,
            spool: SpoolConfig::new(directory.path(), None),
            disable_certificate_check: false,
        };
        let client = crate::agent::client::build_reqwest_client(false).unwrap();
        let terminal = tokio::time::timeout(
            Duration::from_secs(2),
            run_job(
                &config,
                &client,
                &Arc::new(Semaphore::new(1)),
                "job-rejected",
                "job-token",
                WirePcapFilter::All,
                1_699_999_999_000_000,
                1_700_000_001_000_000,
                WireLimits {
                    max_bytes: 8_000_000,
                    scan_timeout_ms: 60_000,
                },
                &CancellationToken::new(),
                oneshot::channel().0,
            ),
        )
        .await
        .expect("upload rejection should not wait for the extraction deadline");

        assert_eq!(terminal.code, PcapResultCode::Error);
        assert_eq!(terminal.upload, PcapUploadStatus::Failed);
        assert!(
            terminal
                .message
                .as_deref()
                .is_some_and(|message| message.contains("502 Bad Gateway"))
        );
        server.abort();
    }

    #[test]
    fn writer_pushes_first_bytes_immediately() {
        let (tx, mut rx) = mpsc::channel(2);
        let mut writer = ChannelWriter {
            tx,
            buffer: Vec::with_capacity(CHUNK_SIZE),
            cancel: CancellationToken::new(),
            first_sent: false,
        };
        writer.write_all(b"first").unwrap();
        assert_eq!(rx.try_recv().unwrap().as_ref(), b"first");
        writer.write_all(b"later").unwrap();
        assert!(rx.try_recv().is_err());
        writer.flush().unwrap();
        assert_eq!(rx.try_recv().unwrap().as_ref(), b"later");
    }

    #[test]
    fn writer_splits_oversized_writes_into_bounded_chunks() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut writer = ChannelWriter {
            tx,
            buffer: Vec::with_capacity(CHUNK_SIZE),
            cancel: CancellationToken::new(),
            first_sent: false,
        };
        writer.write_all(b"first").unwrap();
        let oversized = vec![0xa5; CHUNK_SIZE * 2 + 17];
        writer.write_all(&oversized).unwrap();
        writer.flush().unwrap();

        let mut chunks = Vec::new();
        while let Ok(chunk) = rx.try_recv() {
            assert!(chunk.len() <= CHUNK_SIZE);
            chunks.push(chunk);
        }
        assert_eq!(chunks[0].as_ref(), b"first");
        let reconstructed: Vec<u8> = chunks[1..]
            .iter()
            .flat_map(|chunk| chunk.iter().copied())
            .collect();
        assert_eq!(reconstructed, oversized);
    }
}
