// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Extracted file retrieval API: `GET /api/filestore` streams a file
//! Suricata stored with its file-store output.
//!
//! Extracted files are untrusted content captured off the network, so
//! they are only ever served as opaque attachments named by their
//! digest — never inline and never under the file name seen on the
//! wire.

use std::net::SocketAddr;
use std::sync::Arc;

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
use crate::server::filestore::{self, EventFile, OpenError, Sha256};
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

/// Response headers for a successful download. The content is untrusted:
/// force a download, forbid sniffing, and sandbox it should a browser
/// ever render it anyway.
fn file_headers(audit: &AuditContext, sha256: &Sha256, size: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{sha256}\"")) {
        headers.insert(CONTENT_DISPOSITION, value);
    }
    headers.insert(CONTENT_LENGTH, HeaderValue::from(size));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Ok(value) = HeaderValue::from_str(&audit.source) {
        headers.insert(HeaderName::from_static("x-evebox-file-source"), value);
    }
    headers
}

async fn handle(
    context: &Arc<ServerContext>,
    params: &FileRequestParams,
    user: &str,
    remote: String,
    dry_run: bool,
) -> FileResponseResult {
    let mut audit = AuditContext {
        user: user.to_string(),
        remote,
        event_id: params.event_id.clone().unwrap_or_else(|| "-".to_string()),
        sha256: "-".to_string(),
        source: "-".to_string(),
        dry_run,
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
            if dry_run {
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
        Resolved::Agent(_) => Err(fail(
            &audit,
            StatusCode::NOT_IMPLEMENTED,
            "not-implemented",
            "retrieving files from remote agents is not supported yet",
        )),
    }
}

#[cfg(test)]
mod test {
    use super::*;
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
}
