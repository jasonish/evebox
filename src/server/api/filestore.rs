// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Extracted file retrieval API: `GET /api/filestore` streams a file
//! Suricata stored with its file-store output, and
//! `GET /api/filestore/preview` returns the first 64 KiB of one.
//!
//! Extracted files are untrusted content captured off the network, so
//! downloads are only ever served as opaque attachments named by their
//! digest — never inline as a document and never under the file name
//! seen on the wire. A preview returns raw bytes for the UI to display
//! as data (hex, text, strings); it is never meant for the browser to
//! render, and carries the same no-sniff and sandbox headers.

use crate::agent::protocol::{
    FileResult, FileResultCode, PcapUploadStatus, ServerMessage, WireLimits,
};
use crate::server::agents::AgentEntry;
use crate::server::pcap::tasks::{self, UploadState};
use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, mpsc, oneshot, watch};

use axum::body::Body;
use axum::extract::{ConnectInfo, Extension, Json, Query, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_SECURITY_POLICY, CONTENT_TYPE,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::prelude::*;
use crate::server::ServerContext;
use crate::server::api::pcap::remote_addr;
use crate::server::api::util::{error_response, present};
use crate::server::filestore::{self, EventFile, Lane, OpenError, Sha256};
use crate::server::main::SessionExtractor;
use crate::server::routing::{Resolved, RouteError};

type FileResponseResult = Result<Response, Box<Response>>;

/// Query parameters shared by the download and its pre-flight.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct FileRequestParams {
    /// The event referencing the file. Optional: a bare `sha256`
    /// requests a file by digest alone.
    #[serde(default)]
    pub event_id: Option<String>,
    /// The file's SHA-256. With an event it must be one of the files the
    /// event references, and may be omitted when it references just one.
    #[serde(default)]
    pub sha256: Option<String>,
    /// Optional explicit source name, bypassing routing.
    #[serde(default)]
    pub source: Option<String>,
}

/// How much of a file a preview returns: always the start of the file,
/// never more than this. The rest has to be downloaded.
pub(crate) const PREVIEW_LENGTH: u64 = 64 * 1024;

/// What a request does once the file and its source are resolved.
#[derive(Debug, Clone, Copy)]
enum Mode {
    /// Stream the whole file as an attachment.
    Download,
    /// Pre-flight for a download: resolve and check, but send nothing.
    Validate,
    /// Return the start of the file for in-app display.
    Preview,
}

/// `GET /api/filestore`: stream an extracted file as an attachment.
pub(crate) async fn get_file(
    State(context): State<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Extension(ConnectInfo(remote)): Extension<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Query(params): Query<FileRequestParams>,
) -> Response {
    let user = session.username.clone().unwrap_or_else(|| "-".to_string());
    let remote = remote_addr(&context, &headers, remote);
    match handle(&context, &params, &user, remote, false).await {
        Ok(response) => response,
        Err(response) => *response,
    }
}

/// `GET /api/filestore/validate`: pre-flight for the native download.
/// Performs the same event load, file selection and routing as a real
/// request and, for the local store, checks that the file exists, so a
/// missing file surfaces as a message instead of a failed navigation.
pub(crate) async fn validate_file(
    State(context): State<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Extension(ConnectInfo(remote)): Extension<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Query(params): Query<FileRequestParams>,
) -> Response {
    let user = session.username.clone().unwrap_or_else(|| "-".to_string());
    let remote = remote_addr(&context, &headers, remote);
    match handle(&context, &params, &user, remote, true).await {
        Ok(response) => response,
        Err(response) => *response,
    }
}

/// `GET /api/filestore/preview`: the first [`PREVIEW_LENGTH`] bytes of
/// an extracted file, for the UI's hex, text and strings views. The
/// response is fully buffered, so an agent failure is still a JSON error.
pub(crate) async fn preview_file(
    State(context): State<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
    Extension(ConnectInfo(remote)): Extension<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Query(params): Query<FileRequestParams>,
) -> Response {
    let user = session.username.clone().unwrap_or_else(|| "-".to_string());
    let remote = remote_addr(&context, &headers, remote);
    preview(&context, &params, &user, remote).await
}

async fn preview(
    context: &Arc<ServerContext>,
    params: &FileRequestParams,
    user: &str,
    remote: String,
) -> Response {
    match handle_mode(context, params, user, remote, Mode::Preview).await {
        Ok(response) => response,
        Err(response) => *response,
    }
}

/// `GET /api/filestore/sources`: the source names a request's `source`
/// parameter may select right now.
pub(crate) async fn get_sources(
    State(context): State<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Response {
    #[derive(Serialize)]
    struct Source {
        name: String,
        kind: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        hostname: Option<String>,
    }
    let mut sources = Vec::new();
    if context.filestore.has_local() {
        sources.push(Source {
            name: crate::server::agents::LOCAL_PCAP_SOURCE_NAME.to_string(),
            kind: "server",
            hostname: None,
        });
    }
    sources.extend(
        context
            .agents
            .capable_agents(crate::agent::protocol::CAPABILITY_FILESTORE)
            .into_iter()
            .map(|agent| Source {
                name: agent.name.clone(),
                kind: "agent",
                hostname: Some(agent.hostname.clone()),
            }),
    );
    Json(json!({ "sources": sources })).into_response()
}

/// Fields for the single audit log line emitted per request.
struct AuditContext {
    user: String,
    remote: String,
    event_id: String,
    sha256: String,
    source: String,
    dry_run: bool,
}

impl AuditContext {
    /// The audit line for a served request. The pre-flight only
    /// validates; keeping the audit trail to real downloads logs each
    /// one exactly once.
    fn log(&self, outcome: &str, bytes: Option<u64>) {
        if self.dry_run {
            return;
        }
        let bytes = bytes.map_or_else(|| "-".to_string(), |bytes| bytes.to_string());
        info!(
            "filestore: user={:?} remote={:?} event={:?} sha256={} source={:?} outcome={} bytes={}",
            self.user, self.remote, self.event_id, self.sha256, self.source, outcome, bytes
        );
    }

    /// The audit line for a refused or failed request, with the message
    /// the client was given.
    fn log_failure(&self, outcome: &str, message: &str) {
        if self.dry_run {
            return;
        }
        warn!(
            "filestore: user={:?} remote={:?} event={:?} sha256={} source={:?} outcome={} message={:?}",
            self.user, self.remote, self.event_id, self.sha256, self.source, outcome, message
        );
    }
}

/// Log the audit line with the error code as its outcome and build the
/// error response.
fn fail(audit: &AuditContext, status: StatusCode, code: &str, message: &str) -> Box<Response> {
    audit.log_failure(code, message);
    Box::new(error_response(status, code, message))
}

/// Pick the requested file: the digest asked for, checked against the
/// event's files when there is an event, or the event's only file.
fn select_file(
    audit: &AuditContext,
    params: &FileRequestParams,
    event: Option<&serde_json::Value>,
) -> Result<EventFile, Box<Response>> {
    let requested = match present(&params.sha256) {
        Some(raw) => match Sha256::parse(raw) {
            Some(sha256) => Some(sha256),
            None => {
                return Err(fail(
                    audit,
                    StatusCode::BAD_REQUEST,
                    "bad-sha256",
                    "sha256 must be 64 hexadecimal digits",
                ));
            }
        },
        None => None,
    };
    let Some(event) = event else {
        return match requested {
            Some(sha256) => Ok(EventFile {
                sha256,
                filename: None,
                size: None,
            }),
            None => Err(fail(
                audit,
                StatusCode::BAD_REQUEST,
                "bad-request",
                "an event_id or sha256 is required",
            )),
        };
    };
    let mut files = filestore::event_files(event);
    match requested {
        Some(sha256) => files
            .into_iter()
            .find(|file| file.sha256 == sha256)
            .ok_or_else(|| {
                fail(
                    audit,
                    StatusCode::BAD_REQUEST,
                    "file-not-in-event",
                    "the event does not reference a file with this sha256",
                )
            }),
        None => match files.len() {
            0 => Err(fail(
                audit,
                StatusCode::BAD_REQUEST,
                "no-file",
                "the event does not reference a file with a sha256",
            )),
            1 => Ok(files.pop().expect("one file")),
            _ => {
                let candidates: Vec<&str> = files.iter().map(|file| file.sha256.as_str()).collect();
                let message = "the event references more than one file; choose one by sha256";
                audit.log_failure("ambiguous-file", message);
                Err(Box::new(
                    (
                        StatusCode::CONFLICT,
                        Json(json!({
                            "error": {
                                "code": "ambiguous-file",
                                "message": message,
                                "candidates": candidates,
                            }
                        })),
                    )
                        .into_response(),
                ))
            }
        },
    }
}

fn route_error(audit: &AuditContext, err: RouteError) -> Box<Response> {
    match err {
        RouteError::NoSource(name) => {
            let message = name
                .map(|name| format!("no file source connected for {name}"))
                .unwrap_or_else(|| "no file source is configured or connected".to_string());
            fail(
                audit,
                StatusCode::SERVICE_UNAVAILABLE,
                "no-source",
                &message,
            )
        }
        RouteError::NoRule(sensor) => {
            let message = sensor
                .map(|sensor| format!("no routing rule matches sensor {sensor}"))
                .unwrap_or_else(|| {
                    "no routing rule matches this request and no default source is set".to_string()
                });
            fail(
                audit,
                StatusCode::SERVICE_UNAVAILABLE,
                "no-source",
                &message,
            )
        }
        RouteError::Ambiguous(candidates) => {
            let message = "multiple file sources could serve this request";
            audit.log_failure("ambiguous-source", message);
            Box::new(
                (
                    StatusCode::CONFLICT,
                    Json(json!({
                        "error": {
                            "code": "ambiguous-source",
                            "message": message,
                            "candidates": candidates,
                        }
                    })),
                )
                    .into_response(),
            )
        }
    }
}

fn open_error(audit: &AuditContext, err: OpenError) -> Box<Response> {
    match err {
        OpenError::NotFound => fail(
            audit,
            StatusCode::NOT_FOUND,
            "file-not-found",
            "the file is not in the file store (it was not stored, or has been pruned)",
        ),
        OpenError::NotAFile => {
            warn!(
                "filestore: entry for {} is not a regular file; refusing to serve it",
                audit.sha256
            );
            fail(
                audit,
                StatusCode::NOT_FOUND,
                "file-not-found",
                "the file store entry is not a regular file",
            )
        }
        OpenError::Io(err) => {
            error!("filestore: failed to open {}: {err}", audit.sha256);
            fail(
                audit,
                StatusCode::INTERNAL_SERVER_ERROR,
                "io",
                "failed to read the file store",
            )
        }
    }
}

/// Headers common to every response carrying file bytes. The content is
/// untrusted: forbid sniffing, sandbox it should a browser ever render
/// it anyway, and keep it out of caches.
fn untrusted_bytes_headers(audit: &AuditContext, length: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Ok(value) = HeaderValue::from_str(&audit.source) {
        headers.insert(HeaderName::from_static("x-evebox-file-source"), value);
    }
    headers
}

/// Response headers for a successful download: the untrusted-bytes set,
/// forced to download under the file's digest.
fn file_headers(audit: &AuditContext, sha256: &Sha256, size: u64) -> HeaderMap {
    let mut headers = untrusted_bytes_headers(audit, size);
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{sha256}\"")) {
        headers.insert(CONTENT_DISPOSITION, value);
    }
    headers
}

/// A successful preview: the untrusted-bytes set plus the file's total
/// size. No `Content-Disposition`: the UI fetches the bytes and displays
/// them as data.
fn preview_response(audit: &AuditContext, size: u64, bytes: Vec<u8>) -> Response {
    let mut headers = untrusted_bytes_headers(audit, bytes.len() as u64);
    headers.insert(
        HeaderName::from_static("x-evebox-file-size"),
        HeaderValue::from(size),
    );
    (headers, Body::from(bytes)).into_response()
}

/// Read up to `length` bytes from the start of an open local file.
async fn read_local_prefix(file: tokio::fs::File, length: u64) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(length).read_to_end(&mut bytes).await?;
    Ok(bytes)
}

async fn handle(
    context: &Arc<ServerContext>,
    params: &FileRequestParams,
    user: &str,
    remote: String,
    dry_run: bool,
) -> FileResponseResult {
    let mode = if dry_run {
        Mode::Validate
    } else {
        Mode::Download
    };
    handle_mode(context, params, user, remote, mode).await
}

async fn handle_mode(
    context: &Arc<ServerContext>,
    params: &FileRequestParams,
    user: &str,
    remote: String,
    mode: Mode,
) -> FileResponseResult {
    let mut audit = AuditContext {
        user: user.to_string(),
        remote,
        event_id: params.event_id.clone().unwrap_or_else(|| "-".to_string()),
        sha256: "-".to_string(),
        source: "-".to_string(),
        dry_run: matches!(mode, Mode::Validate),
    };

    let event = match present(&params.event_id) {
        Some(event_id) => match context
            .datastore
            .get_event_by_id(event_id.to_string())
            .await
        {
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
                error!("File request failed to load event {event_id:?}: {err}");
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

    let file = select_file(&audit, params, event_source)?;
    audit.sha256 = file.sha256.to_string();

    let routing = context.pcap.get_routing();
    let source = context
        .filestore
        .resolve_source(
            &context.agents,
            &routing,
            event_source,
            present(&params.source),
        )
        .map_err(|err| route_error(&audit, err))?;
    audit.source = source.name().to_string();

    match source {
        Resolved::Local => {
            let Some(store) = context.filestore.local() else {
                // Resolution only yields Local when a store is configured.
                return Err(fail(
                    &audit,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "no local file store is configured",
                ));
            };
            if matches!(mode, Mode::Preview) {
                let (handle, size) = store
                    .open(&file.sha256)
                    .await
                    .map_err(|err| open_error(&audit, err))?;
                let length = size.min(PREVIEW_LENGTH);
                let bytes = read_local_prefix(handle, length).await.map_err(|err| {
                    error!("filestore: failed to read {}: {err}", audit.sha256);
                    fail(
                        &audit,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "io",
                        "failed to read the file store",
                    )
                })?;
                // The file may have been shorter than its size at open
                // (it never should be); report what was actually read.
                audit.log("preview", Some(bytes.len() as u64));
                return Ok(preview_response(&audit, size, bytes));
            }
            if matches!(mode, Mode::Validate) {
                let size = store
                    .stat(&file.sha256)
                    .await
                    .map_err(|err| open_error(&audit, err))?;
                return Ok(Json(json!({
                    "ok": true,
                    "sha256": file.sha256.as_str(),
                    "filename": file.sha256.as_str(),
                    "size": size,
                    "source": audit.source,
                }))
                .into_response());
            }
            let (handle, size) = store
                .open(&file.sha256)
                .await
                .map_err(|err| open_error(&audit, err))?;
            audit.log("ok", Some(size));
            // Content-Length is the size at open time. Suricata never
            // rewrites a finished file in place (a duplicate only has its
            // mtime bumped), so the length is stable; take no more than
            // announced regardless.
            use tokio::io::AsyncReadExt;
            let body = Body::from_stream(tokio_util::io::ReaderStream::new(handle.take(size)));
            Ok((file_headers(&audit, &file.sha256, size), body).into_response())
        }
        Resolved::Agent(entry) => {
            let preview = match mode {
                Mode::Validate => {
                    return Ok(Json(json!({
                        "ok": true, "sha256": file.sha256.as_str(),
                        "filename": file.sha256.as_str(), "source": audit.source,
                    }))
                    .into_response());
                }
                Mode::Download => false,
                Mode::Preview => true,
            };
            let lane = if preview {
                Lane::Preview
            } else {
                Lane::Download
            };
            let Some(permits) = context.filestore.try_acquire(&entry.name, lane) else {
                return Err(fail(
                    &audit,
                    StatusCode::TOO_MANY_REQUESTS,
                    "source-busy",
                    "file source is busy",
                ));
            };
            if preview {
                preview_agent_file(context, entry, &file.sha256, audit, permits).await
            } else {
                stream_agent_file(context, entry, &file.sha256, audit, permits).await
            }
        }
    }
}

/// A remote job is scoped to the browser response. Dropping an unread or
/// interrupted response cancels it and revokes its one-time upload token.
struct FileJobGuard {
    tasks: Arc<tasks::Registry>,
    entry: Arc<AgentEntry>,
    id: String,
    token: String,
    finished: bool,
}

impl FileJobGuard {
    fn disarm(&mut self) {
        self.finished = true;
    }
}

impl Drop for FileJobGuard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.entry.try_send(ServerMessage::Cancel {
                id: self.id.clone(),
                token: self.token.clone(),
            });
        }
        self.tasks.remove(&self.id, &self.token);
    }
}

struct FileStream {
    body_rx: mpsc::Receiver<Bytes>,
    upload_rx: watch::Receiver<UploadState>,
    result_rx: oneshot::Receiver<FileResult>,
    result: Option<FileResult>,
    size: u64,
    bytes: u64,
    first: Option<Bytes>,
    done: bool,
    guard: FileJobGuard,
    _permits: (OwnedSemaphorePermit, OwnedSemaphorePermit),
}

fn file_result_error(result: &FileResult) -> (&'static str, &'static str) {
    match result.code {
        FileResultCode::NotFound => (
            "file-not-found",
            "the file is not in the file store (it was not stored, or has been pruned)",
        ),
        FileResultCode::Cancelled => ("agent-cancelled", "file agent cancelled the request"),
        _ => ("agent-error", "file agent failed to read the file"),
    }
}

/// The error for a result that is not Complete once the agent has
/// announced the file: not-found now contradicts the announcement and
/// is the agent's error rather than a missing file.
fn late_result_error(result: &FileResult) -> (&'static str, &'static str) {
    let (code, message) = file_result_error(result);
    if result.code == FileResultCode::NotFound {
        ("agent-error", message)
    } else {
        (code, message)
    }
}

/// Check a finished job: the agent's result, the upload and the bytes
/// received must all agree. `size` is the file's announced size and
/// `upload_len` how much of it the job was asked to upload.
fn verify_file_result(
    result: &FileResult,
    upload: &UploadState,
    size: u64,
    upload_len: u64,
    bytes: u64,
) -> std::io::Result<()> {
    if result.code != FileResultCode::Complete
        || result.upload != PcapUploadStatus::Complete
        || result.size != Some(size)
        || result.bytes != upload_len
        || bytes != upload_len
        || !matches!(upload, UploadState::Complete { bytes: uploaded } if *uploaded == upload_len)
    {
        return Err(std::io::Error::other(
            "file agent result, upload and announced size disagree",
        ));
    }
    Ok(())
}

enum FileStartOutcome {
    Ready {
        size: u64,
        early_result: Option<FileResult>,
    },
    Terminal(FileResult),
}

/// The terminal result and the size announcement travel through separate
/// channels. A fast successful upload may finish before the browser task is
/// scheduled: prefer a queued start, and retain an early Complete result so
/// it can still be checked against the upload and announced size.
async fn await_file_start(
    start_rx: &mut oneshot::Receiver<u64>,
    result_rx: &mut oneshot::Receiver<FileResult>,
) -> Result<FileStartOutcome, &'static str> {
    tokio::select! {
        biased;
        start = &mut *start_rx => Ok(FileStartOutcome::Ready {
            size: start.map_err(|_| "file agent did not announce its size")?,
            early_result: None,
        }),
        result = &mut *result_rx => {
            let result = result.map_err(|_| "file agent disconnected")?;
            if result.code == FileResultCode::Complete {
                let size = (&mut *start_rx).await
                    .map_err(|_| "file agent completed without a size announcement")?;
                Ok(FileStartOutcome::Ready { size, early_result: Some(result) })
            } else {
                Ok(FileStartOutcome::Terminal(result))
            }
        }
    }
}

/// A remote file job whose upload has begun: the announced size, the
/// first chunk, and the channels to read the rest from.
struct AgentFileStart {
    size: u64,
    first: Option<Bytes>,
    body_rx: mpsc::Receiver<Bytes>,
    upload_rx: watch::Receiver<UploadState>,
    result_rx: oneshot::Receiver<FileResult>,
    early_result: Option<FileResult>,
    guard: FileJobGuard,
}

enum AgentFileOutcome {
    /// A zero-byte file, already verified against the agent's result.
    Empty,
    /// A non-empty file with its upload under way.
    Started(AgentFileStart),
}

/// Start a remote file job and wait for its upload to begin, so any
/// failure up to that point can still be returned as a JSON error. A
/// nonzero `max_bytes` asks the agent to upload only the file's start
/// and caps what the upload endpoint accepts for the job.
async fn start_agent_file(
    context: &Arc<ServerContext>,
    entry: Arc<AgentEntry>,
    sha256: &Sha256,
    audit: &AuditContext,
    max_bytes: u64,
) -> Result<AgentFileOutcome, Box<Response>> {
    let settings = &context.pcap.settings;
    if !entry.probe_liveness(settings.liveness_timeout).await {
        return Err(fail(
            audit,
            StatusCode::SERVICE_UNAVAILABLE,
            "agent-unresponsive",
            "file agent is not responding",
        ));
    }
    let id = uuid::Uuid::new_v4().to_string();
    use base64::Engine;
    use rand::RngCore;
    let mut secret = [0u8; 32];
    rand::rng().fill_bytes(&mut secret);
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
    let tasks::FileHandles {
        mut body_rx,
        mut upload_rx,
        mut result_rx,
        mut start_rx,
    } = context
        .pcap_tasks
        .register_file(
            id.clone(),
            token.clone(),
            entry.name.clone(),
            entry.generation,
            // The upload endpoint refuses more than was asked for. A
            // whole-file request is bounded by the announced size once
            // it streams.
            if max_bytes == 0 { u64::MAX } else { max_bytes },
        )
        .map_err(|_| {
            fail(
                audit,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "duplicate file job id",
            )
        })?;
    let mut guard = FileJobGuard {
        tasks: context.pcap_tasks.clone(),
        entry: entry.clone(),
        id: id.clone(),
        token: token.clone(),
        finished: false,
    };
    let message = ServerMessage::FileRequest {
        id,
        token,
        sha256: sha256.as_str().to_string(),
        limits: WireLimits {
            max_bytes,
            // The agent's deadline for queueing, opening and announcing
            // the file is how long this side waits for the announcement,
            // as for packet capture.
            scan_timeout_ms: u64::try_from(settings.request_timeout.as_millis())
                .unwrap_or(u64::MAX)
                .max(1),
        },
    };
    if context.agents.try_send_current(&entry, message).is_err() {
        guard.finished = true;
        return Err(fail(
            audit,
            StatusCode::SERVICE_UNAVAILABLE,
            "agent-unavailable",
            "file agent is not accepting requests",
        ));
    }
    let timeout = settings.request_timeout;
    let (size, mut early_result) = match tokio::time::timeout(
        timeout,
        await_file_start(&mut start_rx, &mut result_rx),
    )
    .await
    {
        Ok(Ok(FileStartOutcome::Ready { size, early_result })) => (size, early_result),
        Ok(Ok(FileStartOutcome::Terminal(result))) => {
            if result.code == FileResultCode::NotFound
                && result.upload == PcapUploadStatus::None
                && result.size.is_none()
                && result.bytes == 0
                && matches!(*upload_rx.borrow(), UploadState::Pending)
            {
                return Err(fail(
                    audit,
                    StatusCode::NOT_FOUND,
                    "file-not-found",
                    "the file is not in the file store (it was not stored, or has been pruned)",
                ));
            }
            let (code, message) = late_result_error(&result);
            return Err(fail(audit, StatusCode::BAD_GATEWAY, code, message));
        }
        Ok(Err(message)) => {
            return Err(fail(audit, StatusCode::BAD_GATEWAY, "agent-error", message));
        }
        Err(_) => {
            return Err(fail(
                audit,
                StatusCode::GATEWAY_TIMEOUT,
                "timeout",
                "file agent did not respond in time",
            ));
        }
    };
    // Do not commit a 200 until the upload has begun (or a zero-byte
    // upload has completed). A failed open/upload can still return JSON.
    let mut body_open = true;
    let first = match tokio::time::timeout(timeout, async {
        loop {
            tokio::select! {
                chunk = body_rx.recv(), if body_open => match chunk {
                    Some(chunk) if !chunk.is_empty() => break Ok(Some(chunk)),
                    None => { body_open = false; if size > 0 { break Err(("agent-error", "file upload ended without data")); } },
                    _ => {}
                },
                result = &mut result_rx, if early_result.is_none() => {
                    let result = result.map_err(|_| ("agent-error", "file agent disconnected"))?;
                    if result.code != FileResultCode::Complete { break Err(late_result_error(&result)); }
                    early_result = Some(result);
                    if size == 0 { break Ok(None); }
                },
                changed = upload_rx.changed() => {
                    if changed.is_err() || matches!(*upload_rx.borrow(), UploadState::Failed { .. }) {
                        break Err(("agent-error", "file upload failed"));
                    }
                    if size == 0 && matches!(*upload_rx.borrow(), UploadState::Complete { bytes: 0 }) {
                        break Ok(None);
                    }
                }
            }
        }
    }).await {
        Ok(Ok(first)) => first,
        Ok(Err((code, message))) => return Err(fail(audit, StatusCode::BAD_GATEWAY, code, message)),
        Err(_) => return Err(fail(audit, StatusCode::GATEWAY_TIMEOUT, "timeout", "file agent did not upload in time")),
    };
    if first
        .as_ref()
        .is_some_and(|chunk| chunk.len() as u64 > size)
    {
        return Err(fail(
            audit,
            StatusCode::BAD_GATEWAY,
            "agent-protocol",
            "file upload exceeds announced size",
        ));
    }
    if size == 0 {
        let result = match early_result {
            Some(result) => result,
            None => match tokio::time::timeout(settings.stall_timeout, &mut result_rx).await {
                Ok(Ok(result)) => result,
                _ => {
                    return Err(fail(
                        audit,
                        StatusCode::GATEWAY_TIMEOUT,
                        "timeout",
                        "file agent omitted its result",
                    ));
                }
            },
        };
        if verify_file_result(&result, &upload_rx.borrow(), 0, 0, 0).is_err() {
            return Err(fail(
                audit,
                StatusCode::BAD_GATEWAY,
                "agent-error",
                "file upload and result disagree",
            ));
        }
        guard.disarm();
        return Ok(AgentFileOutcome::Empty);
    }
    Ok(AgentFileOutcome::Started(AgentFileStart {
        size,
        first,
        body_rx,
        upload_rx,
        result_rx,
        early_result,
        guard,
    }))
}

/// Once a remote file's bytes have all arrived, wait for the agent's
/// terminal result and the upload's completion and check they agree
/// with the announced size and the upload length asked for.
async fn finish_agent_file(
    early_result: Option<FileResult>,
    result_rx: &mut oneshot::Receiver<FileResult>,
    upload_rx: &mut watch::Receiver<UploadState>,
    size: u64,
    upload_len: u64,
    bytes: u64,
    stall: std::time::Duration,
) -> std::io::Result<()> {
    let result = if let Some(result) = early_result {
        result
    } else {
        tokio::time::timeout(stall, &mut *result_rx)
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "file result timed out")
            })?
            .map_err(|_| std::io::Error::other("file result channel closed"))?
    };
    // Upload state and result delivery are independent. A clean EOF on
    // the body channel precedes the upload's Complete watch state.
    if !matches!(
        *upload_rx.borrow(),
        UploadState::Complete { .. } | UploadState::Failed { .. }
    ) {
        tokio::time::timeout(stall, upload_rx.changed())
            .await
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "file upload completion timed out",
                )
            })?
            .map_err(|_| std::io::Error::other("upload state channel closed"))?;
    }
    verify_file_result(&result, &upload_rx.borrow(), size, upload_len, bytes)
}

async fn stream_agent_file(
    context: &Arc<ServerContext>,
    entry: Arc<AgentEntry>,
    sha256: &Sha256,
    audit: AuditContext,
    permits: (OwnedSemaphorePermit, OwnedSemaphorePermit),
) -> FileResponseResult {
    let AgentFileStart {
        size,
        first,
        body_rx,
        upload_rx,
        result_rx,
        early_result,
        guard,
    } = match start_agent_file(context, entry, sha256, &audit, 0).await? {
        AgentFileOutcome::Empty => {
            audit.log("ok", Some(0));
            return Ok((file_headers(&audit, sha256, 0), Body::empty()).into_response());
        }
        AgentFileOutcome::Started(start) => start,
    };
    let settings = &context.pcap.settings;
    let stream = FileStream {
        body_rx,
        upload_rx,
        result_rx,
        result: early_result,
        size,
        bytes: 0,
        first,
        done: false,
        guard,
        _permits: permits,
    };
    let stall = settings.stall_timeout;
    let headers = file_headers(&audit, sha256, size);
    // The audit line is written when the transfer ends, so an upload
    // that stalls, fails verification or is abandoned is never recorded
    // as a served download.
    let audit = Arc::new(audit);
    let body = futures::stream::try_unfold(stream, move |mut state| {
        let audit = audit.clone();
        async move {
            if state.done {
                return Ok(None);
            }
            match state.next(stall).await {
                Ok(next) => {
                    if next.is_none() || state.done {
                        state.done = true;
                        audit.log("ok", Some(state.bytes));
                    }
                    Ok(next.map(|chunk| (chunk, state)))
                }
                Err(err) => {
                    let outcome = if err.kind() == std::io::ErrorKind::TimedOut {
                        "timeout"
                    } else {
                        "agent-error"
                    };
                    audit.log_failure(outcome, &err.to_string());
                    Err(err)
                }
            }
        }
    });
    Ok((headers, Body::from_stream(body)).into_response())
}

impl FileStream {
    /// The next chunk for the browser, or `None` at a verified end of
    /// file. `done` is set once the final chunk has been verified and
    /// handed out.
    async fn next(&mut self, stall: std::time::Duration) -> std::io::Result<Option<Bytes>> {
        let stalled = || std::io::Error::new(std::io::ErrorKind::TimedOut, "file upload stalled");
        let next = if let Some(first) = self.first.take() {
            Some(first)
        } else {
            tokio::time::timeout(stall, self.body_rx.recv())
                .await
                .map_err(|_| stalled())?
        };
        let Some(chunk) = next else {
            self.finish(stall).await?;
            return Ok(None);
        };
        self.bytes = self
            .bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| std::io::Error::other("file byte count overflow"))?;
        if self.bytes > self.size {
            return Err(std::io::Error::other("file exceeded announced size"));
        }
        if self.bytes < self.size {
            return Ok(Some(chunk));
        }
        // Hold the final chunk until the upload EOF and terminal result
        // agree. Otherwise a missing result could look like a complete
        // Content-Length response to the browser.
        loop {
            match tokio::time::timeout(stall, self.body_rx.recv()).await {
                Ok(None) => break,
                Ok(Some(extra)) if extra.is_empty() => continue,
                Ok(Some(_)) => {
                    return Err(std::io::Error::other("file exceeded announced size"));
                }
                Err(_) => return Err(stalled()),
            }
        }
        self.finish(stall).await?;
        self.done = true;
        Ok(Some(chunk))
    }

    /// Verify the ended upload against the agent's result and the
    /// announced size, and release the job.
    async fn finish(&mut self, stall: std::time::Duration) -> std::io::Result<()> {
        finish_agent_file(
            self.result.take(),
            &mut self.result_rx,
            &mut self.upload_rx,
            self.size,
            self.size,
            self.bytes,
            stall,
        )
        .await?;
        self.guard.disarm();
        Ok(())
    }
}

/// Preview the start of a remote file. The agent is asked to upload only
/// the first [`PREVIEW_LENGTH`] bytes, and the preview is verified
/// against its result like a download; a failed verification is an
/// error, never partial data.
async fn preview_agent_file(
    context: &Arc<ServerContext>,
    entry: Arc<AgentEntry>,
    sha256: &Sha256,
    audit: AuditContext,
    _permits: (OwnedSemaphorePermit, OwnedSemaphorePermit),
) -> FileResponseResult {
    let AgentFileStart {
        size,
        first,
        mut body_rx,
        mut upload_rx,
        mut result_rx,
        early_result,
        mut guard,
    } = match start_agent_file(context, entry, sha256, &audit, PREVIEW_LENGTH).await? {
        AgentFileOutcome::Empty => {
            audit.log("preview", Some(0));
            return Ok(preview_response(&audit, 0, Vec::new()));
        }
        AgentFileOutcome::Started(start) => start,
    };
    let end = size.min(PREVIEW_LENGTH);
    let stall = context.pcap.settings.stall_timeout;
    let stalled = |audit: &AuditContext| {
        fail(
            audit,
            StatusCode::GATEWAY_TIMEOUT,
            "timeout",
            "file upload stalled",
        )
    };
    let oversize = |audit: &AuditContext| {
        fail(
            audit,
            StatusCode::BAD_GATEWAY,
            "agent-protocol",
            "file upload exceeds the requested length",
        )
    };
    let mut bytes = Vec::with_capacity(end as usize);
    let mut next = first;
    loop {
        let chunk = match next.take() {
            Some(chunk) => chunk,
            None => match tokio::time::timeout(stall, body_rx.recv()).await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => return Err(stalled(&audit)),
            },
        };
        if bytes.len() as u64 + chunk.len() as u64 > end {
            return Err(oversize(&audit));
        }
        bytes.extend_from_slice(&chunk);
    }
    if (bytes.len() as u64) < end {
        return Err(fail(
            &audit,
            StatusCode::BAD_GATEWAY,
            "agent-error",
            "file upload ended early",
        ));
    }
    if let Err(err) = finish_agent_file(
        early_result,
        &mut result_rx,
        &mut upload_rx,
        size,
        end,
        bytes.len() as u64,
        stall,
    )
    .await
    {
        debug!("filestore: preview of {sha256} failed verification: {err}");
        return Err(fail(
            &audit,
            StatusCode::BAD_GATEWAY,
            "agent-error",
            "file upload and result disagree",
        ));
    }
    guard.disarm();
    audit.log("preview", Some(bytes.len() as u64));
    Ok(preview_response(&audit, size, bytes))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn late_results_keep_their_code_and_message() {
        let result = |code| FileResult {
            code,
            upload: PcapUploadStatus::None,
            message: None,
            size: Some(4),
            bytes: 0,
        };
        assert_eq!(
            late_result_error(&result(FileResultCode::Cancelled)),
            ("agent-cancelled", "file agent cancelled the request")
        );
        assert_eq!(
            late_result_error(&result(FileResultCode::Error)),
            ("agent-error", "file agent failed to read the file")
        );
        // Not-found after the size was announced is the agent's error.
        assert_eq!(
            late_result_error(&result(FileResultCode::NotFound)).0,
            "agent-error"
        );
    }

    fn complete_file_result(size: u64) -> FileResult {
        FileResult {
            code: FileResultCode::Complete,
            upload: PcapUploadStatus::Complete,
            message: None,
            size: Some(size),
            bytes: size,
        }
    }

    #[tokio::test]
    async fn completed_result_before_size_announcement_is_preserved() {
        let (start_tx, mut start_rx) = oneshot::channel();
        let (result_tx, mut result_rx) = oneshot::channel();
        result_tx.send(complete_file_result(5)).unwrap();
        let wait =
            tokio::spawn(async move { await_file_start(&mut start_rx, &mut result_rx).await });
        tokio::task::yield_now().await;
        start_tx.send(5).unwrap();
        let FileStartOutcome::Ready {
            size,
            early_result: Some(result),
        } = wait.await.unwrap().unwrap()
        else {
            panic!("early successful result was not preserved");
        };
        assert_eq!(size, 5);
        assert_eq!(result, complete_file_result(5));
    }

    #[tokio::test]
    async fn queued_size_wins_when_both_file_messages_are_ready() {
        let (start_tx, mut start_rx) = oneshot::channel();
        let (result_tx, mut result_rx) = oneshot::channel();
        result_tx.send(complete_file_result(5)).unwrap();
        start_tx.send(5).unwrap();
        let FileStartOutcome::Ready {
            size: 5,
            early_result: None,
        } = await_file_start(&mut start_rx, &mut result_rx)
            .await
            .unwrap()
        else {
            panic!("buffered file start was not preferred");
        };
        assert_eq!(result_rx.await.unwrap(), complete_file_result(5));
    }

    #[test]
    fn remote_file_result_requires_matching_size_and_clean_upload() {
        let result = FileResult {
            code: FileResultCode::Complete,
            upload: PcapUploadStatus::Complete,
            message: None,
            size: Some(5),
            bytes: 5,
        };
        let complete = |bytes| UploadState::Complete { bytes };
        assert!(verify_file_result(&result, &complete(5), 5, 5, 5).is_ok());
        assert!(verify_file_result(&result, &complete(4), 5, 5, 5).is_err());
        assert!(verify_file_result(&result, &complete(5), 5, 5, 4).is_err());
        assert!(
            verify_file_result(
                &result,
                &UploadState::Failed {
                    reason: "io",
                    bytes: 5
                },
                5,
                5,
                5
            )
            .is_err()
        );

        // A job asked for only the start of a file: the result keeps the
        // file's size and reports the shorter upload.
        let prefix = FileResult {
            size: Some(9),
            bytes: 5,
            ..result.clone()
        };
        assert!(verify_file_result(&prefix, &complete(5), 9, 5, 5).is_ok());
        assert!(verify_file_result(&prefix, &complete(5), 5, 5, 5).is_err());
        assert!(verify_file_result(&prefix, &complete(5), 9, 9, 5).is_err());
        assert!(verify_file_result(&result, &complete(5), 9, 5, 5).is_err());
    }
    use std::path::Path;

    use axum::body::to_bytes;
    use tokio::sync::Mutex;

    use crate::eventrepo::EventRepo;
    use crate::server::filestore::{FilestoreService, LocalFilestore};
    use crate::server::metrics::Metrics;
    use crate::server::{ServerConfig, ServerContext};
    use crate::sqlite::connection::{ConnectionBuilder, init_event_db};
    use crate::sqlite::eventrepo::SqliteEventRepo;

    const SHA: &str = "a3c5f1e2d4b6a8c0e1f3a5b7c9d0e2f4a6b8c0d1e3f5a7b9c1d2e4f6a8b0c2d4";
    const OTHER: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    async fn build_repo(db_path: &Path) -> EventRepo {
        let builder = ConnectionBuilder::filename(Some(db_path));
        let mut writer = builder.open_connection(true).await.unwrap();
        init_event_db(&mut writer).await.unwrap();
        let pool = builder.open_pool(false).await.unwrap();
        let writer = Arc::new(Mutex::new(writer));
        let repo = SqliteEventRepo::new(writer, pool, Arc::new(Metrics::default()));
        EventRepo::SQLite(repo)
    }

    /// A context with the given events ingested and a local store at
    /// `<dir>/filestore` holding `SHA` with content `hello`.
    async fn context(dir: &Path, events: Vec<serde_json::Value>) -> Arc<ServerContext> {
        let datastore = build_repo(&dir.join("events.sqlite")).await;
        let mut sink = datastore.get_importer().unwrap();
        for event in events {
            sink.submit(event).await.unwrap();
        }
        sink.commit().await.unwrap();
        let configdb = crate::sqlite::configdb::open(Some(&dir.join("config.sqlite")))
            .await
            .unwrap();
        let mut context = ServerContext::new(
            ServerConfig::default(),
            Arc::new(configdb),
            datastore,
            Arc::new(Metrics::default()),
        );
        let store_dir = dir.join("filestore");
        std::fs::create_dir_all(store_dir.join(&SHA[..2])).unwrap();
        std::fs::write(store_dir.join(&SHA[..2]).join(SHA), b"hello").unwrap();
        context.filestore = Arc::new(FilestoreService::new(Some(LocalFilestore::new(store_dir))));
        Arc::new(context)
    }

    fn fileinfo_event(sha256: &str) -> serde_json::Value {
        json!({
            "timestamp": "2023-11-14T22:15:20.000000+0000",
            "event_type": "fileinfo",
            "src_ip": "10.1.1.5",
            "dest_ip": "192.0.2.10",
            "host": "test-sensor",
            "fileinfo": {
                "filename": "/evil.exe",
                "sha256": sha256,
                "size": 5,
                "stored": true,
            },
        })
    }

    /// SQLite event ids are row ids: the first ingested event is "1".
    async fn event_id(_context: &ServerContext) -> String {
        "1".to_string()
    }

    async fn request(context: &Arc<ServerContext>, params: FileRequestParams) -> Response {
        match handle(context, &params, "tester", "127.0.0.1".to_string(), false).await {
            Ok(response) => response,
            Err(response) => *response,
        }
    }

    async fn error_code(response: Response) -> String {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        json["error"]["code"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn downloads_the_file_referenced_by_an_event() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path(), vec![fileinfo_event(SHA)]).await;
        let id = event_id(&context).await;
        let response = request(
            &context,
            FileRequestParams {
                event_id: Some(id),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(headers[CONTENT_TYPE], "application/octet-stream");
        assert_eq!(
            headers[CONTENT_DISPOSITION],
            format!("attachment; filename=\"{SHA}\"").as_str()
        );
        assert_eq!(headers[CONTENT_LENGTH], "5");
        assert_eq!(headers[X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers["x-evebox-file-source"], "(server)");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), b"hello");
    }

    #[tokio::test]
    async fn downloads_by_digest_alone() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path(), vec![]).await;
        let response = request(
            &context,
            FileRequestParams {
                sha256: Some(SHA.to_ascii_uppercase()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn request_errors_are_structured() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path(), vec![fileinfo_event(SHA)]).await;
        let id = event_id(&context).await;

        let response = request(
            &context,
            FileRequestParams {
                sha256: Some("../../etc/passwd".to_string()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error_code(response).await, "bad-sha256");

        let response = request(
            &context,
            FileRequestParams {
                event_id: Some(id),
                sha256: Some(OTHER.to_string()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error_code(response).await, "file-not-in-event");

        let response = request(
            &context,
            FileRequestParams {
                sha256: Some(OTHER.to_string()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(error_code(response).await, "file-not-found");

        let response = request(&context, FileRequestParams::default()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = request(
            &context,
            FileRequestParams {
                sha256: Some(SHA.to_string()),
                source: Some("sensor-x".to_string()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error_code(response).await, "no-source");
    }

    #[tokio::test]
    async fn event_with_several_files_requires_a_choice() {
        let dir = tempfile::tempdir().unwrap();
        let mut event = fileinfo_event(SHA);
        event["event_type"] = json!("alert");
        event["files"] = json!([{ "filename": "/other", "sha256": OTHER }]);
        let context = context(dir.path(), vec![event]).await;
        let id = event_id(&context).await;

        let response = request(
            &context,
            FileRequestParams {
                event_id: Some(id.clone()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(error_code(response).await, "ambiguous-file");

        let response = request(
            &context,
            FileRequestParams {
                event_id: Some(id),
                sha256: Some(SHA.to_string()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn validate_reports_size_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path(), vec![]).await;
        let validate = |sha256: &str| {
            let params = FileRequestParams {
                sha256: Some(sha256.to_string()),
                ..Default::default()
            };
            let context = context.clone();
            async move {
                match handle(&context, &params, "tester", "-".to_string(), true).await {
                    Ok(response) => response,
                    Err(response) => *response,
                }
            }
        };
        let response = validate(SHA).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["size"], 5);
        assert_eq!(json["sha256"], SHA);

        let response = validate(OTHER).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// A second stored file, `BIG`, of `len` bytes where byte `i` is
    /// `i % 251`, so any slice is recognisable.
    const BIG: &str = "b1c5f1e2d4b6a8c0e1f3a5b7c9d0e2f4a6b8c0d1e3f5a7b9c1d2e4f6a8b0c2d4";

    fn big_content(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    fn store_big(dir: &Path, len: usize) -> Vec<u8> {
        let content = big_content(len);
        let shard = dir.join("filestore").join(&BIG[..2]);
        std::fs::create_dir_all(&shard).unwrap();
        std::fs::write(shard.join(BIG), &content).unwrap();
        content
    }

    fn by_digest(sha256: &str) -> FileRequestParams {
        FileRequestParams {
            sha256: Some(sha256.to_string()),
            ..Default::default()
        }
    }

    async fn preview_request(context: &Arc<ServerContext>, params: FileRequestParams) -> Response {
        preview(context, &params, "tester", "127.0.0.1".to_string()).await
    }

    #[tokio::test]
    async fn preview_returns_a_whole_small_local_file() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path(), vec![fileinfo_event(SHA)]).await;
        let id = event_id(&context).await;
        let response = preview_request(
            &context,
            FileRequestParams {
                event_id: Some(id),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(headers[CONTENT_TYPE], "application/octet-stream");
        assert!(headers.get(CONTENT_DISPOSITION).is_none());
        assert_eq!(headers[CONTENT_LENGTH], "5");
        assert_eq!(headers[X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers[CONTENT_SECURITY_POLICY], "sandbox");
        assert_eq!(headers[CACHE_CONTROL], "no-store");
        assert_eq!(headers["x-evebox-file-source"], "(server)");
        assert_eq!(headers["x-evebox-file-size"], "5");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), b"hello");
    }

    #[tokio::test]
    async fn preview_is_capped_to_the_start_of_a_local_file() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path(), vec![]).await;
        let len = PREVIEW_LENGTH as usize + 4096;
        let content = store_big(dir.path(), len);
        let response = preview_request(&context, by_digest(BIG)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-evebox-file-size"], len.to_string());
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), &content[..PREVIEW_LENGTH as usize]);

        // Range parameters are not part of the API and change nothing.
        let uri: axum::http::Uri = format!("/?sha256={BIG}&offset=70000&length=1000")
            .parse()
            .unwrap();
        let Query(params) = Query::<FileRequestParams>::try_from_uri(&uri).unwrap();
        let response = preview_request(&context, params).await;
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), &content[..PREVIEW_LENGTH as usize]);
    }

    #[tokio::test]
    async fn preview_errors_match_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path(), vec![fileinfo_event(SHA)]).await;
        let id = event_id(&context).await;
        let cases = [
            (
                by_digest("../../etc/passwd"),
                StatusCode::BAD_REQUEST,
                "bad-sha256",
            ),
            (
                FileRequestParams {
                    event_id: Some(id),
                    sha256: Some(OTHER.to_string()),
                    ..Default::default()
                },
                StatusCode::BAD_REQUEST,
                "file-not-in-event",
            ),
            (by_digest(OTHER), StatusCode::NOT_FOUND, "file-not-found"),
            (
                FileRequestParams {
                    sha256: Some(SHA.to_string()),
                    source: Some("sensor-x".to_string()),
                    ..Default::default()
                },
                StatusCode::SERVICE_UNAVAILABLE,
                "no-source",
            ),
        ];
        for (params, status, code) in cases {
            let response = preview_request(&context, params).await;
            assert_eq!(response.status(), status);
            assert_eq!(error_code(response).await, code);
        }
    }
}
