// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! In-flight remote packet-capture tasks.
//!
//! The WebSocket terminal result and the HTTP upload are independent inputs.
//! A browser request owns the receivers returned by [`Registry::register`];
//! the agent upload and control channel find the matching senders by job id
//! and one-time token. A job lives no longer than the control connection it
//! was dispatched on: a disconnect fails the waiting request, and the user
//! simply retries.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, watch};

use crate::agent::protocol::{
    AgentMessage, FileResult, FileResultCode, PcapResult, PcapResultCode, PcapUploadStatus,
    WireStats,
};
use crate::server::agents::{AgentConnectionId, AgentMessageHandler};

const BODY_CHANNEL_CAPACITY: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UploadState {
    Pending,
    Streaming,
    Complete { bytes: u64 },
    Failed { reason: &'static str, bytes: u64 },
}

struct Task {
    token: String,
    agent: String,
    generation: u64,
    max_bytes: u64,
    body_tx: Mutex<Option<mpsc::Sender<Bytes>>>,
    upload_tx: watch::Sender<UploadState>,
    result_tx: Mutex<Option<ResultSender>>,
    start_tx: Mutex<Option<oneshot::Sender<u64>>>,
    uploading: AtomicBool,
    file: bool,
    upload_allowed: AtomicBool,
}

enum ResultSender {
    Pcap(oneshot::Sender<PcapResult>),
    File(oneshot::Sender<FileResult>),
}

pub(crate) struct Handles {
    pub(crate) body_rx: mpsc::Receiver<Bytes>,
    pub(crate) upload_rx: watch::Receiver<UploadState>,
    pub(crate) result_rx: oneshot::Receiver<PcapResult>,
}

pub(crate) struct FileHandles {
    pub(crate) body_rx: mpsc::Receiver<Bytes>,
    pub(crate) upload_rx: watch::Receiver<UploadState>,
    pub(crate) result_rx: oneshot::Receiver<FileResult>,
    pub(crate) start_rx: oneshot::Receiver<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegisterError {
    DuplicateId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UploadSendError {
    ReceiverClosed,
    TooLarge,
}

/// The single authorized upload for a job.
pub(crate) struct UploadSink {
    tx: Option<mpsc::Sender<Bytes>>,
    state: watch::Sender<UploadState>,
    max_bytes: u64,
    bytes: u64,
    finished: bool,
}

impl UploadSink {
    pub(crate) async fn send(&mut self, chunk: Bytes) -> Result<(), UploadSendError> {
        let len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
        let Some(total) = self.bytes.checked_add(len) else {
            return Err(UploadSendError::TooLarge);
        };
        if total > self.max_bytes {
            return Err(UploadSendError::TooLarge);
        }
        self.tx
            .as_ref()
            .expect("unfinished upload always owns its body sender")
            .send(chunk)
            .await
            .map_err(|_| UploadSendError::ReceiverClosed)?;
        self.bytes = total;
        Ok(())
    }

    pub(crate) fn complete(mut self) {
        self.finished = true;
        // Close the body channel before publishing completion, so Complete
        // always means that the upload reached a clean EOF.
        drop(self.tx.take());
        let _ = self.state.send(UploadState::Complete { bytes: self.bytes });
    }

    pub(crate) fn fail(mut self, reason: &'static str) {
        self.finished = true;
        drop(self.tx.take());
        let _ = self.state.send(UploadState::Failed {
            reason,
            bytes: self.bytes,
        });
    }
}

impl Drop for UploadSink {
    fn drop(&mut self) {
        if !self.finished {
            drop(self.tx.take());
            let _ = self.state.send(UploadState::Failed {
                reason: "agent-upload-ended",
                bytes: self.bytes,
            });
        }
    }
}

#[derive(Default)]
pub(crate) struct Registry {
    tasks: RwLock<HashMap<String, Arc<Task>>>,
}

impl Registry {
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.tasks.read().unwrap().len()
    }

    // The consuming WS suite is compiled out on Windows.
    #[cfg(all(test, not(windows)))]
    pub(crate) fn is_uploading(&self, id: &str) -> bool {
        self.tasks
            .read()
            .unwrap()
            .get(id)
            .is_some_and(|task| task.uploading.load(Ordering::Acquire))
    }

    pub(crate) fn register(
        &self,
        id: String,
        token: String,
        agent: String,
        generation: u64,
        max_bytes: u64,
    ) -> Result<Handles, RegisterError> {
        let (result_tx, result_rx) = oneshot::channel();
        let (body_rx, upload_rx) = self.insert(
            id,
            token,
            agent,
            generation,
            max_bytes,
            ResultSender::Pcap(result_tx),
            None,
        )?;
        Ok(Handles {
            body_rx,
            upload_rx,
            result_rx,
        })
    }

    pub(crate) fn register_file(
        &self,
        id: String,
        token: String,
        agent: String,
        generation: u64,
        max_bytes: u64,
    ) -> Result<FileHandles, RegisterError> {
        let (result_tx, result_rx) = oneshot::channel();
        let (start_tx, start_rx) = oneshot::channel();
        let (body_rx, upload_rx) = self.insert(
            id,
            token,
            agent,
            generation,
            max_bytes,
            ResultSender::File(result_tx),
            Some(start_tx),
        )?;
        Ok(FileHandles {
            body_rx,
            upload_rx,
            result_rx,
            start_rx,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn insert(
        &self,
        id: String,
        token: String,
        agent: String,
        generation: u64,
        max_bytes: u64,
        result_tx: ResultSender,
        start_tx: Option<oneshot::Sender<u64>>,
    ) -> Result<(mpsc::Receiver<Bytes>, watch::Receiver<UploadState>), RegisterError> {
        let (body_tx, body_rx) = mpsc::channel(BODY_CHANNEL_CAPACITY);
        let (upload_tx, upload_rx) = watch::channel(UploadState::Pending);
        let mut tasks = self.tasks.write().unwrap();
        if tasks.contains_key(&id) {
            return Err(RegisterError::DuplicateId);
        }
        let file = matches!(result_tx, ResultSender::File(_));
        tasks.insert(
            id,
            Arc::new(Task {
                token,
                agent,
                generation,
                max_bytes,
                body_tx: Mutex::new(Some(body_tx)),
                upload_tx,
                result_tx: Mutex::new(Some(result_tx)),
                start_tx: Mutex::new(start_tx),
                file,
                uploading: AtomicBool::new(false),
                upload_allowed: AtomicBool::new(true),
            }),
        );
        Ok((body_rx, upload_rx))
    }

    /// Remove exactly one token-bound task. A late result or upload for a
    /// removed job is silently ignored.
    pub(crate) fn remove(&self, id: &str, token: &str) {
        let mut tasks = self.tasks.write().unwrap();
        if tasks
            .get(id)
            .is_some_and(|task| token_matches(&task.token, token))
        {
            tasks.remove(id);
        }
    }

    pub(crate) fn begin_upload(&self, id: &str, token: &str) -> Option<UploadSink> {
        self.begin_typed_upload(id, token, false)
    }

    pub(crate) fn begin_file_upload(&self, id: &str, token: &str) -> Option<UploadSink> {
        self.begin_typed_upload(id, token, true)
    }

    fn begin_typed_upload(&self, id: &str, token: &str, file: bool) -> Option<UploadSink> {
        let task = self.tasks.read().unwrap().get(id)?.clone();
        if task.file != file || !token_matches(&task.token, token) {
            return None;
        }
        // Serialize upload start with result delivery and disconnect
        // handling: either the upload claims the body sender first, or a
        // result/disconnect blocks a late upload for a finished request.
        let mut body_tx = task.body_tx.lock().unwrap();
        if !task.upload_allowed.load(Ordering::Acquire) {
            return None;
        }
        let tx = body_tx.take()?;
        task.uploading.store(true, Ordering::Release);
        // Publish Streaming before releasing the same lock used by result
        // delivery. A result accepted after upload start must never become
        // visible while the upload watch still says Pending.
        let _ = task.upload_tx.send(UploadState::Streaming);
        drop(body_tx);
        Some(UploadSink {
            tx: Some(tx),
            state: task.upload_tx.clone(),
            max_bytes: task.max_bytes,
            bytes: 0,
            finished: false,
        })
    }

    /// The task `id` names, when it is the kind of job `file` says and
    /// is bound to `connection` and `token`: a job belongs to the exact
    /// connection it was dispatched on.
    fn bound_task(
        &self,
        connection: &AgentConnectionId,
        id: &str,
        token: &str,
        file: bool,
    ) -> Option<Arc<Task>> {
        let task = self.tasks.read().unwrap().get(id).cloned()?;
        (task.file == file
            && task.agent == connection.name
            && task.generation == connection.generation
            && token_matches(&task.token, token))
        .then_some(task)
    }

    fn deliver_result(
        &self,
        connection: &AgentConnectionId,
        id: &str,
        token: &str,
        result: PcapResult,
    ) {
        let Some(task) = self.bound_task(connection, id, token, false) else {
            return;
        };
        let body_tx_guard = task.body_tx.lock().unwrap();
        if !(result.code == PcapResultCode::Complete && result.upload == PcapUploadStatus::Complete)
        {
            // Only a result which explicitly promises a separate upload may
            // be followed by a result-before-upload start.
            task.upload_allowed.store(false, Ordering::Release);
        }
        drop(body_tx_guard);
        if let Some(ResultSender::Pcap(tx)) = task.result_tx.lock().unwrap().take() {
            let _ = tx.send(result);
        }
    }

    /// The announced size is the whole file's, which a job asking for
    /// only the file's start may well exceed; `max_bytes` bounds the
    /// upload, not the announcement.
    fn file_start(&self, connection: &AgentConnectionId, id: &str, token: &str, size: u64) {
        let Some(task) = self.bound_task(connection, id, token, true) else {
            return;
        };
        if let Some(tx) = task.start_tx.lock().unwrap().take() {
            let _ = tx.send(size);
        }
    }

    fn deliver_file_result(
        &self,
        connection: &AgentConnectionId,
        id: &str,
        token: &str,
        result: FileResult,
    ) {
        let Some(task) = self.bound_task(connection, id, token, true) else {
            return;
        };
        let _body_tx = task.body_tx.lock().unwrap();
        if !(result.code == FileResultCode::Complete && result.upload == PcapUploadStatus::Complete)
        {
            task.upload_allowed.store(false, Ordering::Release);
        }
        if let Some(ResultSender::File(tx)) = task.result_tx.lock().unwrap().take() {
            let _ = tx.send(result);
        }
    }

    fn fail_pending(&self, connection: &AgentConnectionId) {
        let tasks = self.tasks.read().unwrap();
        for task in tasks.values() {
            if task.agent != connection.name || task.generation != connection.generation {
                continue;
            }
            // The job died with its control channel. Reject a late upload
            // and fail the waiting browser request; an already-finished
            // request has consumed its result sender, making this a no-op.
            let _body_tx = task.body_tx.lock().unwrap();
            task.upload_allowed.store(false, Ordering::Release);
            if let Some(tx) = task.result_tx.lock().unwrap().take() {
                match tx {
                    ResultSender::Pcap(tx) => {
                        let _ = tx.send(PcapResult {
                            code: PcapResultCode::Error,
                            upload: PcapUploadStatus::Failed,
                            message: Some("agent disconnected".to_string()),
                            stats: None,
                        });
                    }
                    ResultSender::File(tx) => {
                        let _ = tx.send(FileResult {
                            code: FileResultCode::Error,
                            upload: PcapUploadStatus::Failed,
                            message: Some("agent disconnected".to_string()),
                            size: None,
                            bytes: 0,
                        });
                    }
                }
            }
        }
    }
}

impl AgentMessageHandler for Registry {
    fn message(&self, connection: &AgentConnectionId, message: AgentMessage) {
        match message {
            AgentMessage::PcapResult { id, token, result } => {
                self.deliver_result(connection, &id, &token, result)
            }
            AgentMessage::FileResult { id, token, result } => {
                self.deliver_file_result(connection, &id, &token, result)
            }
            AgentMessage::FileStart { id, token, size } => {
                self.file_start(connection, &id, &token, size)
            }
            _ => {}
        }
    }

    fn disconnected(&self, connection: &AgentConnectionId) {
        self.fail_pending(connection);
    }
}

fn token_matches(expected: &str, presented: &str) -> bool {
    let (expected, presented) = (expected.as_bytes(), presented.as_bytes());
    if expected.len() != presented.len() {
        return false;
    }
    expected
        .iter()
        .zip(presented)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

/// The verified meaning of an agent's terminal result for the waiting
/// browser request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RemoteOutcome {
    /// Not terminal yet: the result promises an upload whose clean EOF (or
    /// buffered body tail) has not been consumed.
    Wait,
    /// Extraction complete and every byte count agrees.
    Complete {
        stats: WireStats,
    },
    NoCandidateFiles,
    NoMatch,
    Cancelled {
        message: Option<String>,
    },
    /// The agent reported an extraction or upload failure.
    Error {
        message: String,
    },
    /// The result contradicts the observed upload — a bug in the peer.
    Protocol {
        detail: String,
    },
}

/// The one interpreter of an agent terminal result against the observed
/// upload state. `streamed` is the byte count already handed to the browser
/// response (0 before the first chunk); `body_open` is whether upload body
/// chunks may still arrive.
pub(crate) fn classify_result(
    result: &PcapResult,
    upload: &UploadState,
    body_open: bool,
    streamed: u64,
) -> RemoteOutcome {
    match result.code {
        PcapResultCode::Complete => {
            if result.upload == PcapUploadStatus::Failed
                || matches!(upload, UploadState::Failed { .. })
            {
                return RemoteOutcome::Error {
                    message: result
                        .message
                        .clone()
                        .unwrap_or_else(|| "pcap upload failed".to_string()),
                };
            }
            let Some(stats) = result.stats else {
                return RemoteOutcome::Protocol {
                    detail: "complete result omitted extraction statistics".to_string(),
                };
            };
            match result.upload {
                PcapUploadStatus::None => {
                    if !matches!(upload, UploadState::Pending) || streamed != 0 {
                        return RemoteOutcome::Protocol {
                            detail: "complete result did not confirm its upload".to_string(),
                        };
                    }
                    // The only successful extraction which produces no HTTP
                    // upload is a zero-byte limit truncation: the writer
                    // could not hand off even the first capture byte.
                    if stats.bytes != 0 || !stats.truncated {
                        return RemoteOutcome::Protocol {
                            detail: "complete result without an upload was not an empty truncation"
                                .to_string(),
                        };
                    }
                    RemoteOutcome::Complete { stats }
                }
                PcapUploadStatus::Complete => {
                    // The promised upload EOF, and any body tail buffered
                    // behind it, must be consumed before the result is
                    // terminal.
                    let UploadState::Complete { bytes: uploaded } = *upload else {
                        return RemoteOutcome::Wait;
                    };
                    if body_open {
                        return RemoteOutcome::Wait;
                    }
                    if uploaded != streamed || stats.bytes != streamed {
                        return RemoteOutcome::Protocol {
                            detail: format!(
                                "byte counts disagree: streamed={streamed} uploaded={uploaded} reported={}",
                                stats.bytes
                            ),
                        };
                    }
                    RemoteOutcome::Complete { stats }
                }
                PcapUploadStatus::Failed => unreachable!("handled above"),
            }
        }
        PcapResultCode::NoCandidateFiles | PcapResultCode::NoMatch => {
            if result.upload != PcapUploadStatus::None
                || !matches!(upload, UploadState::Pending)
                || streamed != 0
            {
                return RemoteOutcome::Protocol {
                    detail: "empty result arrived after an upload had started".to_string(),
                };
            }
            if result.code == PcapResultCode::NoCandidateFiles {
                RemoteOutcome::NoCandidateFiles
            } else {
                RemoteOutcome::NoMatch
            }
        }
        PcapResultCode::Cancelled | PcapResultCode::Error => {
            // An extraction can fail or be cancelled after its upload body
            // already reached clean EOF, so a completed upload is a
            // legitimate companion to either code. Denying that any upload
            // happened after one was observed is not.
            if result.upload == PcapUploadStatus::None && !matches!(upload, UploadState::Pending) {
                return RemoteOutcome::Protocol {
                    detail: "terminal result denies the observed upload".to_string(),
                };
            }
            if result.code == PcapResultCode::Cancelled {
                RemoteOutcome::Cancelled {
                    message: result.message.clone(),
                }
            } else {
                RemoteOutcome::Error {
                    message: result
                        .message
                        .clone()
                        .unwrap_or_else(|| "pcap agent extraction failed".to_string()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register(tasks: &Registry) -> Handles {
        tasks
            .register(
                "job-1".to_string(),
                "secret".to_string(),
                "sensor-a".to_string(),
                7,
                8,
            )
            .unwrap()
    }

    fn result_message(
        id: &str,
        token: &str,
        code: PcapResultCode,
        upload: PcapUploadStatus,
    ) -> AgentMessage {
        AgentMessage::PcapResult {
            id: id.to_string(),
            token: token.to_string(),
            result: PcapResult {
                code,
                upload,
                message: None,
                stats: None,
            },
        }
    }

    #[tokio::test]
    async fn file_start_and_result_are_connection_bound_and_uploads_are_typed() {
        let tasks = Registry::default();
        let mut handles = tasks
            .register_file("file-1".into(), "secret".into(), "sensor-a".into(), 7, 8)
            .unwrap();
        let correct = AgentConnectionId {
            name: "sensor-a".into(),
            generation: 7,
        };
        let old = AgentConnectionId {
            name: "sensor-a".into(),
            generation: 6,
        };
        tasks.message(
            &old,
            AgentMessage::FileStart {
                id: "file-1".into(),
                token: "secret".into(),
                size: 4,
            },
        );
        tasks.message(
            &correct,
            AgentMessage::FileStart {
                id: "file-1".into(),
                token: "wrong".into(),
                size: 4,
            },
        );
        assert!(handles.start_rx.try_recv().is_err());
        tasks.message(
            &correct,
            AgentMessage::FileStart {
                id: "file-1".into(),
                token: "secret".into(),
                size: 4,
            },
        );
        assert_eq!(handles.start_rx.await.unwrap(), 4);
        assert!(tasks.begin_upload("file-1", "secret").is_none());
        let mut sink = tasks.begin_file_upload("file-1", "secret").unwrap();
        sink.send(Bytes::from_static(b"data")).await.unwrap();
        sink.complete();
        assert_eq!(
            handles.body_rx.recv().await.unwrap(),
            Bytes::from_static(b"data")
        );
        tasks.message(
            &old,
            AgentMessage::FileResult {
                id: "file-1".into(),
                token: "secret".into(),
                result: FileResult {
                    code: FileResultCode::Complete,
                    upload: PcapUploadStatus::Complete,
                    message: None,
                    size: Some(4),
                    bytes: 4,
                },
            },
        );
        assert!(handles.result_rx.try_recv().is_err());
        tasks.message(
            &correct,
            AgentMessage::FileResult {
                id: "file-1".into(),
                token: "secret".into(),
                result: FileResult {
                    code: FileResultCode::Complete,
                    upload: PcapUploadStatus::Complete,
                    message: None,
                    size: Some(4),
                    bytes: 4,
                },
            },
        );
        assert_eq!(handles.result_rx.await.unwrap().bytes, 4);
    }

    #[tokio::test]
    async fn file_start_announces_the_whole_size_while_the_upload_is_capped() {
        let tasks = Registry::default();
        let handles = tasks
            .register_file("file-1".into(), "secret".into(), "sensor-a".into(), 7, 4)
            .unwrap();
        let connection = AgentConnectionId {
            name: "sensor-a".into(),
            generation: 7,
        };
        // A request for the start of a large file: the announcement is
        // the file's size, well beyond the 4-byte upload cap.
        tasks.message(
            &connection,
            AgentMessage::FileStart {
                id: "file-1".into(),
                token: "secret".into(),
                size: 4096,
            },
        );
        assert_eq!(handles.start_rx.await.unwrap(), 4096);
        let mut sink = tasks.begin_file_upload("file-1", "secret").unwrap();
        sink.send(Bytes::from_static(b"data")).await.unwrap();
        assert_eq!(
            sink.send(Bytes::from_static(b"more")).await,
            Err(UploadSendError::TooLarge)
        );
    }

    #[test]
    fn token_comparison_matches_equality() {
        assert!(token_matches("secret", "secret"));
        assert!(!token_matches("secret", "Secret"));
        assert!(!token_matches("secret", "secret-longer"));
    }

    #[tokio::test]
    async fn upload_is_token_bound_single_use_and_size_bounded() {
        let tasks = Registry::default();
        let mut handles = register(&tasks);
        assert!(tasks.begin_upload("job-1", "wrong").is_none());
        let mut sink = tasks.begin_upload("job-1", "secret").unwrap();
        assert!(tasks.begin_upload("job-1", "secret").is_none());

        sink.send(Bytes::from_static(b"pcap")).await.unwrap();
        assert_eq!(
            handles.body_rx.recv().await.unwrap(),
            Bytes::from_static(b"pcap")
        );
        assert_eq!(
            sink.send(Bytes::from_static(b"too-long")).await,
            Err(UploadSendError::TooLarge)
        );
        sink.complete();
        handles.upload_rx.changed().await.unwrap();
        assert_eq!(
            *handles.upload_rx.borrow(),
            UploadState::Complete { bytes: 4 }
        );
    }

    #[tokio::test]
    async fn upload_completion_preserves_the_queued_body_tail() {
        let tasks = Registry::default();
        let mut handles = tasks
            .register(
                "job-1".to_string(),
                "secret".to_string(),
                "sensor-a".to_string(),
                7,
                16,
            )
            .unwrap();
        let mut sink = tasks.begin_upload("job-1", "secret").unwrap();
        sink.send(Bytes::from_static(b"first")).await.unwrap();
        sink.send(Bytes::from_static(b"second")).await.unwrap();
        sink.complete();

        assert_eq!(
            *handles.upload_rx.borrow(),
            UploadState::Complete { bytes: 11 }
        );
        assert_eq!(
            handles.body_rx.recv().await.unwrap(),
            Bytes::from_static(b"first")
        );
        assert_eq!(
            handles.body_rx.recv().await.unwrap(),
            Bytes::from_static(b"second")
        );
        assert!(handles.body_rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn duplicate_results_deliver_once_and_require_the_exact_connection() {
        let tasks = Registry::default();
        let handles = register(&tasks);
        let connection = AgentConnectionId {
            name: "sensor-a".to_string(),
            generation: 7,
        };
        let message = || {
            result_message(
                "job-1",
                "secret",
                PcapResultCode::Complete,
                PcapUploadStatus::None,
            )
        };

        // Results from another name or generation are not this job's.
        tasks.message(
            &AgentConnectionId {
                name: "sensor-b".to_string(),
                generation: 7,
            },
            message(),
        );
        tasks.message(
            &AgentConnectionId {
                name: "sensor-a".to_string(),
                generation: 8,
            },
            message(),
        );
        tasks.message(&connection, message());
        tasks.message(&connection, message());
        assert_eq!(
            handles.result_rx.await.unwrap().code,
            PcapResultCode::Complete
        );
    }

    #[tokio::test]
    async fn result_may_arrive_before_its_promised_upload() {
        let tasks = Registry::default();
        let mut handles = register(&tasks);
        let connection = AgentConnectionId {
            name: "sensor-a".to_string(),
            generation: 7,
        };
        tasks.message(
            &connection,
            result_message(
                "job-1",
                "secret",
                PcapResultCode::Complete,
                PcapUploadStatus::Complete,
            ),
        );

        // A result which promises a separate completed upload must not block
        // that upload from starting afterwards.
        let sink = tasks.begin_upload("job-1", "secret").unwrap();
        sink.complete();
        assert!(handles.body_rx.recv().await.is_none());
        assert_eq!(
            *handles.upload_rx.borrow(),
            UploadState::Complete { bytes: 0 }
        );
    }

    #[test]
    fn result_only_terminal_state_rejects_a_late_upload() {
        let tasks = Registry::default();
        let _handles = register(&tasks);
        tasks.message(
            &AgentConnectionId {
                name: "sensor-a".to_string(),
                generation: 7,
            },
            result_message(
                "job-1",
                "secret",
                PcapResultCode::NoMatch,
                PcapUploadStatus::None,
            ),
        );
        assert!(tasks.begin_upload("job-1", "secret").is_none());
    }

    #[test]
    fn removal_is_token_bound_and_late_results_are_ignored() {
        let tasks = Registry::default();
        let _handles = register(&tasks);
        tasks.remove("job-1", "wrong");
        assert!(tasks.begin_upload("job-1", "secret").is_some());
        tasks.remove("job-1", "secret");
        assert_eq!(tasks.len(), 0);

        // A late result for the removed job is silently dropped.
        tasks.message(
            &AgentConnectionId {
                name: "sensor-a".to_string(),
                generation: 7,
            },
            result_message(
                "job-1",
                "secret",
                PcapResultCode::Cancelled,
                PcapUploadStatus::None,
            ),
        );
    }

    #[test]
    fn duplicate_task_ids_are_rejected_and_cleanup_is_token_bound() {
        let tasks = Registry::default();
        let _handles = register(&tasks);
        assert!(matches!(
            tasks.register(
                "job-1".to_string(),
                "other".to_string(),
                "sensor-a".to_string(),
                8,
                8,
            ),
            Err(RegisterError::DuplicateId)
        ));
        tasks.remove("job-1", "other");
        assert!(tasks.begin_upload("job-1", "secret").is_some());
    }

    #[tokio::test]
    async fn disconnect_fails_only_its_own_generation() {
        let tasks = Registry::default();
        let pending = register(&tasks);
        tasks.disconnected(&AgentConnectionId {
            name: "sensor-a".to_string(),
            generation: 6,
        });
        let mut pending_rx = pending.result_rx;
        assert!(matches!(
            pending_rx.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));

        tasks.disconnected(&AgentConnectionId {
            name: "sensor-a".to_string(),
            generation: 7,
        });
        assert_eq!(pending_rx.await.unwrap().code, PcapResultCode::Error);
        assert!(tasks.begin_upload("job-1", "secret").is_none());
    }

    #[tokio::test]
    async fn disconnect_fails_a_job_mid_upload() {
        let tasks = Registry::default();
        let handles = register(&tasks);
        let _sink = tasks.begin_upload("job-1", "secret").unwrap();
        tasks.disconnected(&AgentConnectionId {
            name: "sensor-a".to_string(),
            generation: 7,
        });
        assert_eq!(handles.result_rx.await.unwrap().code, PcapResultCode::Error);
    }

    fn complete_result(upload: PcapUploadStatus, stats: WireStats) -> PcapResult {
        PcapResult {
            code: PcapResultCode::Complete,
            upload,
            message: None,
            stats: Some(stats),
        }
    }

    #[test]
    fn classifier_verifies_a_promised_upload() {
        let result = complete_result(
            PcapUploadStatus::Complete,
            WireStats {
                bytes: 128,
                ..Default::default()
            },
        );

        // Not terminal until upload EOF is observed and the buffered body
        // tail has been drained.
        assert_eq!(
            classify_result(&result, &UploadState::Streaming, true, 64),
            RemoteOutcome::Wait
        );
        assert_eq!(
            classify_result(&result, &UploadState::Complete { bytes: 128 }, true, 64),
            RemoteOutcome::Wait
        );
        assert!(matches!(
            classify_result(&result, &UploadState::Complete { bytes: 128 }, false, 128),
            RemoteOutcome::Complete { stats } if stats.bytes == 128
        ));
        // Any byte-count disagreement is a protocol error.
        assert!(matches!(
            classify_result(&result, &UploadState::Complete { bytes: 128 }, false, 96),
            RemoteOutcome::Protocol { .. }
        ));
    }

    #[test]
    fn classifier_accepts_only_an_empty_truncation_without_an_upload() {
        let truncated = complete_result(
            PcapUploadStatus::None,
            WireStats {
                truncated: true,
                ..Default::default()
            },
        );
        assert!(matches!(
            classify_result(&truncated, &UploadState::Pending, true, 0),
            RemoteOutcome::Complete { .. }
        ));

        let unconfirmed = complete_result(PcapUploadStatus::None, WireStats::default());
        assert!(matches!(
            classify_result(&unconfirmed, &UploadState::Pending, true, 0),
            RemoteOutcome::Protocol { .. }
        ));
        // Data was streamed but the result does not confirm an upload.
        assert!(matches!(
            classify_result(&truncated, &UploadState::Streaming, true, 64),
            RemoteOutcome::Protocol { .. }
        ));
    }

    #[test]
    fn classifier_rejects_inconsistent_results() {
        // Complete without statistics.
        let missing_stats = PcapResult {
            code: PcapResultCode::Complete,
            upload: PcapUploadStatus::Complete,
            message: None,
            stats: None,
        };
        assert!(matches!(
            classify_result(&missing_stats, &UploadState::Pending, true, 0),
            RemoteOutcome::Protocol { .. }
        ));

        // No-match after upload data arrived.
        let no_match = PcapResult {
            code: PcapResultCode::NoMatch,
            upload: PcapUploadStatus::None,
            message: None,
            stats: Some(WireStats::default()),
        };
        assert_eq!(
            classify_result(&no_match, &UploadState::Pending, true, 0),
            RemoteOutcome::NoMatch
        );
        assert!(matches!(
            classify_result(&no_match, &UploadState::Streaming, true, 64),
            RemoteOutcome::Protocol { .. }
        ));

        // An error result denying an upload the server observed.
        let error_denying_upload = PcapResult {
            code: PcapResultCode::Error,
            upload: PcapUploadStatus::None,
            message: None,
            stats: None,
        };
        assert!(matches!(
            classify_result(&error_denying_upload, &UploadState::Streaming, true, 64),
            RemoteOutcome::Protocol { .. }
        ));
    }

    #[test]
    fn classifier_keeps_the_error_from_a_failure_after_upload_eof() {
        // An extraction that failed after its upload already reached clean
        // EOF is an error, not a protocol violation, and the agent's
        // message must survive.
        let error_after_upload = PcapResult {
            code: PcapResultCode::Error,
            upload: PcapUploadStatus::Complete,
            message: Some("open next rotation file: too many open files".to_string()),
            stats: None,
        };
        assert!(matches!(
            classify_result(
                &error_after_upload,
                &UploadState::Complete { bytes: 128 },
                false,
                128
            ),
            RemoteOutcome::Error { message } if message.contains("too many open files")
        ));

        let cancelled_after_upload = PcapResult::cancelled(PcapUploadStatus::Complete, None);
        assert_eq!(
            classify_result(
                &cancelled_after_upload,
                &UploadState::Complete { bytes: 128 },
                false,
                128
            ),
            RemoteOutcome::Cancelled { message: None }
        );
    }

    #[test]
    fn classifier_maps_failures_and_cancellations() {
        let failed_upload = complete_result(
            PcapUploadStatus::Failed,
            WireStats {
                bytes: 64,
                ..Default::default()
            },
        );
        assert!(matches!(
            classify_result(&failed_upload, &UploadState::Streaming, true, 32),
            RemoteOutcome::Error { .. }
        ));

        let cancelled = PcapResult::cancelled(PcapUploadStatus::None, None);
        assert_eq!(
            classify_result(&cancelled, &UploadState::Pending, true, 0),
            RemoteOutcome::Cancelled { message: None }
        );

        let error = PcapResult::error("spool unreadable".to_string());
        assert!(matches!(
            classify_result(&error, &UploadState::Pending, true, 0),
            RemoteOutcome::Error { message } if message == "spool unreadable"
        ));
    }
}
