// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::config::Config;
use crate::eve::filters::EveFilterChain;
use crate::importer::EventSink;
use crate::prelude::*;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::task::JoinHandle;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SocketType {
    #[default]
    UnixStream,
    UnixDgram,
}

impl SocketType {
    #[cfg(not(unix))]
    fn as_str(self) -> &'static str {
        match self {
            Self::UnixStream => "unix_stream",
            Self::UnixDgram => "unix_dgram",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SocketInput {
    path: PathBuf,
    socket_type: SocketType,
    mode: Option<u32>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SocketInputValue {
    Path(PathBuf),
    Detailed(DetailedSocketInput),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailedSocketInput {
    #[serde(alias = "filename")]
    path: PathBuf,
    #[serde(alias = "filetype", default, rename = "type")]
    socket_type: SocketType,
    mode: Option<SocketModeValue>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SocketModeValue {
    Number(u32),
    Octal(String),
}

pub(crate) fn get_inputs(config: &Config) -> Result<Vec<SocketInput>> {
    let values = config
        .get_value::<Vec<SocketInputValue>>("input.sockets")
        .context("failed to read input.sockets from the configuration file")?
        .unwrap_or_default();
    validate_inputs(values)
}

fn validate_inputs(values: Vec<SocketInputValue>) -> Result<Vec<SocketInput>> {
    let mut paths = HashSet::new();
    let mut inputs = Vec::with_capacity(values.len());
    for value in values {
        let input = match value {
            SocketInputValue::Path(path) => SocketInput {
                path,
                socket_type: SocketType::UnixStream,
                mode: None,
            },
            SocketInputValue::Detailed(value) => {
                let mode = value.mode.map(parse_mode).transpose()?;
                SocketInput {
                    path: value.path,
                    socket_type: value.socket_type,
                    mode,
                }
            }
        };
        if input.path.as_os_str().is_empty() {
            bail!("input.sockets contains an empty path");
        }
        if !paths.insert(input.path.clone()) {
            bail!(
                "input.sockets contains duplicate path {}",
                input.path.display()
            );
        }
        inputs.push(input);
    }
    Ok(inputs)
}

fn parse_mode(value: SocketModeValue) -> Result<u32> {
    let mode = match value {
        SocketModeValue::Number(mode) => mode,
        SocketModeValue::Octal(value) => {
            let value = value
                .strip_prefix("0o")
                .or_else(|| value.strip_prefix("0O"))
                .unwrap_or(value.as_str());
            u32::from_str_radix(value, 8)
                .with_context(|| format!("invalid octal socket mode {value:?}"))?
        }
    };
    if mode > 0o7777 {
        bail!("socket mode {mode:#o} is outside the supported permission range");
    }
    Ok(mode)
}

#[cfg(unix)]
pub(crate) fn spawn(
    input: SocketInput,
    sink: EventSink,
    filters: EveFilterChain,
) -> Result<JoinHandle<()>> {
    imp::spawn(input, sink, filters)
}

#[cfg(not(unix))]
pub(crate) fn spawn(
    input: SocketInput,
    _sink: EventSink,
    _filters: EveFilterChain,
) -> Result<JoinHandle<()>> {
    let _ = input.mode;
    bail!(
        "{} socket input {} is not supported on this platform",
        input.socket_type.as_str(),
        input.path.display()
    )
}

#[cfg(unix)]
mod imp {
    use super::*;
    use crate::eve::filters::AddAgentFilenameFilter;
    use std::io::ErrorKind;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::{UnixDatagram, UnixListener, UnixStream};
    use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
    use tokio::time::{Interval, MissedTickBehavior};

    const DEFAULT_BATCH_SIZE: usize = 100;
    const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
    const REPORT_INTERVAL: Duration = Duration::from_secs(60);
    const MAX_RECORD_SIZE: usize = 8 * 1024 * 1024;
    const INPUT_QUEUE_RECORD_CAPACITY: usize = 4096;
    const INPUT_QUEUE_BYTE_CAPACITY: usize = 16 * 1024 * 1024;

    pub(super) fn spawn(
        input: SocketInput,
        sink: EventSink,
        mut filters: EveFilterChain,
    ) -> Result<JoinHandle<()>> {
        filters.add_filter(AddAgentFilenameFilter::new(
            input.path.display().to_string(),
        ));
        let (ingress, records) = SocketIngress::new(input.path.clone());
        let processor =
            SocketProcessor::new(input.path.clone(), sink, filters, ingress.counters.clone());

        match input.socket_type {
            SocketType::UnixStream => {
                let socket = BoundStreamSocket::bind(&input.path, input.mode)?;
                info!(
                    "Listening for Suricata EVE unix_stream input on {}",
                    input.path.display()
                );
                Ok(tokio::spawn(async move {
                    tokio::join!(run_stream(socket, ingress), processor.run(records));
                }))
            }
            SocketType::UnixDgram => {
                let socket = BoundDatagramSocket::bind(&input.path, input.mode)?;
                info!(
                    "Listening for Suricata EVE unix_dgram input on {}",
                    input.path.display()
                );
                Ok(tokio::spawn(async move {
                    tokio::join!(run_datagram(socket, ingress), processor.run(records));
                }))
            }
        }
    }

    #[derive(Default)]
    struct IngressCounters {
        errors: AtomicU64,
        queue_drops: AtomicU64,
    }

    struct QueuedRecord {
        record: u64,
        bytes: Vec<u8>,
        byte_permit: OwnedSemaphorePermit,
    }

    struct SocketIngress {
        path: PathBuf,
        sender: mpsc::Sender<QueuedRecord>,
        byte_budget: Arc<Semaphore>,
        counters: Arc<IngressCounters>,
    }

    impl SocketIngress {
        fn new(path: PathBuf) -> (Self, mpsc::Receiver<QueuedRecord>) {
            let (sender, receiver) = mpsc::channel(INPUT_QUEUE_RECORD_CAPACITY);
            let counters = Arc::new(IngressCounters::default());
            (
                Self {
                    path,
                    sender,
                    byte_budget: Arc::new(Semaphore::new(INPUT_QUEUE_BYTE_CAPACITY)),
                    counters,
                },
                receiver,
            )
        }

        fn enqueue(&self, record: u64, bytes: &[u8]) -> bool {
            if bytes.iter().all(u8::is_ascii_whitespace) {
                return true;
            }

            let byte_permit = match self
                .byte_budget
                .clone()
                .try_acquire_many_owned(bytes.len() as u32)
            {
                Ok(permit) => permit,
                Err(_) => {
                    self.note_queue_drop();
                    return true;
                }
            };

            match self.sender.try_reserve() {
                Ok(slot) => {
                    slot.send(QueuedRecord {
                        record,
                        bytes: bytes.to_vec(),
                        byte_permit,
                    });
                    true
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    drop(byte_permit);
                    self.note_queue_drop();
                    true
                }
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            }
        }

        fn note_error(&self) {
            self.counters.errors.fetch_add(1, Ordering::Relaxed);
        }

        fn note_queue_drop(&self) {
            let total = self.counters.queue_drops.fetch_add(1, Ordering::Relaxed) + 1;
            if total == 1 || total.is_power_of_two() {
                warn!(
                    socket = ?self.path,
                    queue_drops = total,
                    "EVE socket input queue is full; dropping event"
                );
            }
        }
    }

    struct SocketProcessor {
        path: PathBuf,
        sink: EventSink,
        filters: EveFilterChain,
        ingress_counters: Arc<IngressCounters>,
        events: u64,
        commits: u64,
        errors: u64,
        last_ingress_errors: u64,
        last_queue_drops: u64,
        last_report: Instant,
    }

    impl SocketProcessor {
        fn new(
            path: PathBuf,
            sink: EventSink,
            filters: EveFilterChain,
            ingress_counters: Arc<IngressCounters>,
        ) -> Self {
            Self {
                path,
                sink,
                filters,
                ingress_counters,
                events: 0,
                commits: 0,
                errors: 0,
                last_ingress_errors: 0,
                last_queue_drops: 0,
                last_report: Instant::now(),
            }
        }

        async fn run(mut self, mut records: mpsc::Receiver<QueuedRecord>) {
            let mut interval = maintenance_interval();
            // Consume the interval's immediate first tick.
            interval.tick().await;

            loop {
                tokio::select! {
                    queued = records.recv() => {
                        match queued {
                            Some(queued) => self.process_record(queued).await,
                            None => {
                                self.flush().await;
                                return;
                            }
                        }
                    }
                    _ = interval.tick() => self.maintenance().await,
                }
            }
        }

        async fn process_record(&mut self, queued: QueuedRecord) {
            let QueuedRecord {
                record,
                bytes,
                byte_permit,
            } = queued;
            let event = serde_json::from_slice::<serde_json::Value>(&bytes);
            drop(bytes);
            drop(byte_permit);

            match event {
                Ok(event) if event.is_object() => self.submit(event).await,
                Ok(_) => self.invalid_record(record),
                Err(err) => self.parse_error(record, err),
            }
        }

        async fn submit(&mut self, mut event: serde_json::Value) {
            self.filters.run(&mut event);
            self.events += 1;
            match self.sink.submit(event).await {
                Ok(commit) => {
                    if commit || self.sink.pending() >= DEFAULT_BATCH_SIZE {
                        self.commit().await;
                    }
                }
                Err(err) => {
                    self.errors += 1;
                    error!(
                        "Failed to submit EVE event from socket {}: {err:#}",
                        self.path.display()
                    );
                }
            }
        }

        async fn maintenance(&mut self) {
            self.flush().await;
            if self.last_report.elapsed() >= REPORT_INTERVAL {
                let ingress_errors_total = self.ingress_counters.errors.load(Ordering::Relaxed);
                let ingress_errors = ingress_errors_total.saturating_sub(self.last_ingress_errors);
                let queue_drops_total = self.ingress_counters.queue_drops.load(Ordering::Relaxed);
                let queue_drops = queue_drops_total.saturating_sub(self.last_queue_drops);
                debug!(
                    socket = ?self.path,
                    events = self.events,
                    commits = self.commits,
                    errors = self.errors + ingress_errors,
                    queue_drops,
                    queue_drops_total,
                    "EVE socket input activity"
                );
                self.events = 0;
                self.commits = 0;
                self.errors = 0;
                self.last_ingress_errors = ingress_errors_total;
                self.last_queue_drops = queue_drops_total;
                self.last_report = Instant::now();
            }
        }

        async fn flush(&mut self) {
            if self.sink.pending() > 0 {
                self.commit().await;
            }
        }

        async fn commit(&mut self) {
            loop {
                match self.sink.commit().await {
                    Ok(_) => {
                        self.commits += 1;
                        return;
                    }
                    Err(err) => {
                        error!(
                            "Failed to commit EVE events from socket {} (will retry): {err:#}",
                            self.path.display()
                        );
                        tokio::time::sleep(FLUSH_INTERVAL).await;
                    }
                }
            }
        }

        fn parse_error(&mut self, record: u64, err: serde_json::Error) {
            self.errors += 1;
            error!(
                "Failed to parse EVE event from socket {} at record {}: {}",
                self.path.display(),
                record,
                err
            );
        }

        fn invalid_record(&mut self, record: u64) {
            self.errors += 1;
            warn!(
                socket = ?self.path,
                record,
                "Discarding EVE socket record whose top-level JSON value is not an object"
            );
        }
    }

    fn maintenance_interval() -> Interval {
        let mut interval = tokio::time::interval(FLUSH_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval
    }

    async fn run_stream(socket: BoundStreamSocket, ingress: SocketIngress) {
        loop {
            let stream = match socket.listener.accept().await {
                Ok((stream, _)) => stream,
                Err(err) => {
                    error!(
                        "Failed to accept EVE unix_stream connection on {}: {}",
                        socket.path.path.display(),
                        err
                    );
                    ingress.note_error();
                    tokio::time::sleep(FLUSH_INTERVAL).await;
                    continue;
                }
            };

            info!(
                "Suricata connected to EVE unix_stream input {}",
                socket.path.path.display()
            );
            if !read_stream(stream, &ingress).await {
                return;
            }
            info!(
                "Suricata disconnected from EVE unix_stream input {}; waiting for a new connection",
                socket.path.path.display()
            );
        }
    }

    async fn read_stream(stream: UnixStream, ingress: &SocketIngress) -> bool {
        let mut reader = BufReader::new(stream);
        let mut buffer = Vec::new();
        let mut record = 0;
        let mut interval = maintenance_interval();
        interval.tick().await;

        loop {
            tokio::select! {
                result = reader.read_until(b'\n', &mut buffer) => {
                    match result {
                        Ok(0) => {
                            if !buffer.is_empty() {
                                if buffer.len() > MAX_RECORD_SIZE {
                                    warn!(
                                        "EVE record on unix_stream socket {} exceeds {} bytes; closing the connection",
                                        ingress.path.display(),
                                        MAX_RECORD_SIZE
                                    );
                                    ingress.note_error();
                                    return true;
                                }
                                record += 1;
                                if !ingress.enqueue(record, &buffer) {
                                    return false;
                                }
                            }
                            return true;
                        }
                        Ok(_) => {
                            if buffer.len() > MAX_RECORD_SIZE {
                                warn!(
                                    "EVE record on unix_stream socket {} exceeds {} bytes; closing the connection",
                                    ingress.path.display(),
                                    MAX_RECORD_SIZE
                                );
                                ingress.note_error();
                                return true;
                            }
                            record += 1;
                            if !ingress.enqueue(record, &buffer) {
                                return false;
                            }
                            buffer.clear();
                        }
                        Err(err) => {
                            warn!(
                                "Failed to read EVE unix_stream socket {}: {}",
                                ingress.path.display(),
                                err
                            );
                            ingress.note_error();
                            return true;
                        }
                    }
                }
                _ = interval.tick() => {
                    if buffer.len() > MAX_RECORD_SIZE {
                        warn!(
                            "EVE record on unix_stream socket {} exceeds {} bytes; closing the connection",
                            ingress.path.display(),
                            MAX_RECORD_SIZE
                        );
                        ingress.note_error();
                        return true;
                    }
                }
            }
        }
    }

    async fn run_datagram(socket: BoundDatagramSocket, ingress: SocketIngress) {
        // One extra byte makes oversized/truncated datagrams unambiguous at the limit.
        let mut buffer = vec![0; MAX_RECORD_SIZE + 1];
        let mut record = 0;

        loop {
            match socket.socket.recv(&mut buffer).await {
                Ok(0) => {}
                Ok(size) if size > MAX_RECORD_SIZE => {
                    ingress.note_error();
                    warn!(
                        "EVE datagram on socket {} exceeds {} bytes and was discarded",
                        ingress.path.display(),
                        MAX_RECORD_SIZE
                    );
                }
                Ok(size) => {
                    record += 1;
                    if !ingress.enqueue(record, &buffer[..size]) {
                        return;
                    }
                }
                Err(err) => {
                    ingress.note_error();
                    warn!(
                        "Failed to read EVE unix_dgram socket {}: {}",
                        ingress.path.display(),
                        err
                    );
                    tokio::time::sleep(FLUSH_INTERVAL).await;
                }
            }
        }
    }

    struct BoundStreamSocket {
        listener: UnixListener,
        path: SocketPath,
    }

    impl BoundStreamSocket {
        fn bind(path: &Path, mode: Option<u32>) -> Result<Self> {
            prepare_path(path)?;
            let listener = UnixListener::bind(path)
                .with_context(|| format!("failed to bind unix_stream socket {}", path.display()))?;
            let path = match SocketPath::capture(path) {
                Ok(path) => path,
                Err(err) => {
                    let _ = std::fs::remove_file(path);
                    return Err(err);
                }
            };
            set_mode(&path.path, mode)?;
            Ok(Self { listener, path })
        }
    }

    struct BoundDatagramSocket {
        socket: UnixDatagram,
        _path: SocketPath,
    }

    impl BoundDatagramSocket {
        fn bind(path: &Path, mode: Option<u32>) -> Result<Self> {
            prepare_path(path)?;
            let socket = UnixDatagram::bind(path)
                .with_context(|| format!("failed to bind unix_dgram socket {}", path.display()))?;
            let path = match SocketPath::capture(path) {
                Ok(path) => path,
                Err(err) => {
                    let _ = std::fs::remove_file(path);
                    return Err(err);
                }
            };
            set_mode(&path.path, mode)?;
            Ok(Self {
                socket,
                _path: path,
            })
        }
    }

    struct SocketPath {
        path: PathBuf,
        device: u64,
        inode: u64,
    }

    impl SocketPath {
        fn capture(path: &Path) -> Result<Self> {
            let metadata = std::fs::symlink_metadata(path)
                .with_context(|| format!("failed to inspect socket {}", path.display()))?;
            if !metadata.file_type().is_socket() {
                bail!("bound path {} is not a socket", path.display());
            }
            Ok(Self {
                path: path.to_path_buf(),
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
    }

    impl Drop for SocketPath {
        fn drop(&mut self) {
            let metadata = match std::fs::symlink_metadata(&self.path) {
                Ok(metadata) => metadata,
                Err(err) if err.kind() == ErrorKind::NotFound => return,
                Err(err) => {
                    warn!(
                        "Failed to inspect Unix socket {} during cleanup: {}",
                        self.path.display(),
                        err
                    );
                    return;
                }
            };
            if metadata.file_type().is_socket()
                && metadata.dev() == self.device
                && metadata.ino() == self.inode
                && let Err(err) = std::fs::remove_file(&self.path)
                && err.kind() != ErrorKind::NotFound
            {
                warn!(
                    "Failed to remove Unix socket {}: {}",
                    self.path.display(),
                    err
                );
            }
        }
    }

    fn prepare_path(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create parent directory for Unix socket {}",
                    path.display()
                )
            })?;
        }
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_socket() => {
                std::fs::remove_file(path).with_context(|| {
                    format!("failed to remove stale Unix socket {}", path.display())
                })?;
            }
            Ok(_) => {
                bail!(
                    "refusing to replace non-socket path configured in input.sockets: {}",
                    path.display()
                );
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to inspect socket path {}", path.display()));
            }
        }
        Ok(())
    }

    fn set_mode(path: &Path, mode: Option<u32>) -> Result<()> {
        if let Some(mode) = mode {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).with_context(
                || format!("failed to set mode {mode:#o} on socket {}", path.display()),
            )?;
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Write;
        use tokio::io::AsyncWriteExt;

        const RECORD: &[u8] =
            br#"{"timestamp":"2026-08-13T12:00:00.000000+0000","event_type":"stats"}"#;

        #[test]
        fn regular_file_is_never_replaced() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("eve.sock");
            std::fs::File::create(&path)
                .unwrap()
                .write_all(b"keep me")
                .unwrap();

            let err = BoundStreamSocket::bind(&path, None).err().unwrap();
            assert!(err.to_string().contains("refusing to replace non-socket"));
            assert_eq!(std::fs::read(&path).unwrap(), b"keep me");
        }

        #[tokio::test]
        async fn stale_socket_is_replaced_and_owned_socket_is_removed() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("eve.sock");
            drop(std::os::unix::net::UnixListener::bind(&path).unwrap());

            let socket = BoundStreamSocket::bind(&path, None).unwrap();
            assert!(path.exists());
            drop(socket);
            assert!(!path.exists());
        }

        #[tokio::test]
        async fn cleanup_does_not_remove_a_replacement_socket() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("eve.sock");
            let socket = BoundStreamSocket::bind(&path, None).unwrap();

            std::fs::remove_file(&path).unwrap();
            let replacement = std::os::unix::net::UnixListener::bind(&path).unwrap();
            drop(socket);

            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_socket()
            );
            drop(replacement);
        }

        #[tokio::test]
        async fn socket_parent_and_configured_mode_are_created() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("run/eve.sock");
            let socket = BoundStreamSocket::bind(&path, Some(0o620)).unwrap();
            let mode = std::fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o620);
            drop(socket);
        }

        #[test]
        fn full_ingress_queue_counts_dropped_records() {
            let (ingress, _records) = SocketIngress::new(PathBuf::from("eve.sock"));
            for record in 0..INPUT_QUEUE_RECORD_CAPACITY {
                assert!(ingress.enqueue(record as u64, RECORD));
            }

            assert!(ingress.enqueue(INPUT_QUEUE_RECORD_CAPACITY as u64, RECORD));
            assert_eq!(ingress.counters.queue_drops.load(Ordering::Relaxed), 1);
        }

        #[tokio::test]
        async fn stream_socket_receives_a_json_record() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("eve.sock");
            let socket = BoundStreamSocket::bind(&path, None).unwrap();
            let mut sender = UnixStream::connect(&path).await.unwrap();
            sender.write_all(RECORD).await.unwrap();
            sender.write_all(b"\n").await.unwrap();

            let (stream, _) = socket.listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut buffer = Vec::new();
            reader.read_until(b'\n', &mut buffer).await.unwrap();
            let event: serde_json::Value = serde_json::from_slice(&buffer).unwrap();
            assert_eq!(event["event_type"], "stats");
        }

        #[tokio::test]
        async fn datagram_socket_receives_one_json_event() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("eve.sock");
            let socket = BoundDatagramSocket::bind(&path, None).unwrap();
            let sender = UnixDatagram::unbound().unwrap();
            sender.connect(&path).unwrap();
            sender.send(RECORD).await.unwrap();

            let mut buffer = vec![0; 4096];
            let size = socket.socket.recv(&mut buffer).await.unwrap();
            let event: serde_json::Value = serde_json::from_slice(&buffer[..size]).unwrap();
            assert_eq!(event["event_type"], "stats");
        }

        #[tokio::test]
        async fn non_object_datagram_does_not_stop_low_volume_input() {
            let dir = tempfile::tempdir().unwrap();
            let database = dir.path().join("events.sqlite");
            let mut conn = crate::sqlite::connection::open_connection(Some(database), true)
                .await
                .unwrap();
            sqlx::query(
                r#"
                CREATE TABLE events (
                    timestamp INTEGER NOT NULL,
                    archived INTEGER DEFAULT 0,
                    history JSON DEFAULT '[]',
                    source JSON,
                    source_values TEXT
                )
                "#,
            )
            .execute(&mut conn)
            .await
            .unwrap();
            let conn = Arc::new(tokio::sync::Mutex::new(conn));
            let sink = EventSink::SQLite(crate::sqlite::importer::SqliteEventSink::new(
                conn.clone(),
                Arc::new(crate::server::metrics::Metrics::default()),
            ));
            let path = dir.path().join("eve.sock");
            let input = SocketInput {
                path: path.clone(),
                socket_type: SocketType::UnixDgram,
                mode: None,
            };
            let task = spawn(input, sink, EveFilterChain::with_defaults()).unwrap();

            let sender = UnixDatagram::unbound().unwrap();
            sender.connect(&path).unwrap();
            sender.send(b"[]").await.unwrap();
            sender.send(RECORD).await.unwrap();

            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let count: i64 = {
                        let mut conn = conn.lock().await;
                        sqlx::query_scalar("SELECT COUNT(*) FROM events")
                            .fetch_one(&mut *conn)
                            .await
                            .unwrap()
                    };
                    if count == 1 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .unwrap();

            assert!(!task.is_finished());
            task.abort();
            let _ = task.await;
            assert!(!path.exists());
        }

        #[tokio::test]
        async fn datagram_input_keeps_receiving_while_commit_is_blocked() {
            const RECORDS_WHILE_BLOCKED: usize = 512;

            let dir = tempfile::tempdir().unwrap();
            let database = dir.path().join("events.sqlite");
            let mut connection = crate::sqlite::connection::open_connection(Some(database), true)
                .await
                .unwrap();
            sqlx::query(
                r#"
                CREATE TABLE events (
                    timestamp INTEGER NOT NULL,
                    archived INTEGER DEFAULT 0,
                    history JSON DEFAULT '[]',
                    source JSON,
                    source_values TEXT
                )
                "#,
            )
            .execute(&mut connection)
            .await
            .unwrap();
            let connection = Arc::new(tokio::sync::Mutex::new(connection));
            let sink = EventSink::SQLite(crate::sqlite::importer::SqliteEventSink::new(
                connection.clone(),
                Arc::new(crate::server::metrics::Metrics::default()),
            ));

            // Holding the connection lock makes the first timed flush wait in commit.
            let connection_guard = connection.lock().await;
            let path = dir.path().join("eve.sock");
            let input = SocketInput {
                path: path.clone(),
                socket_type: SocketType::UnixDgram,
                mode: None,
            };
            let task = spawn(input, sink, EveFilterChain::with_defaults()).unwrap();
            let sender = UnixDatagram::unbound().unwrap();
            sender.connect(&path).unwrap();
            sender.send(RECORD).await.unwrap();

            tokio::time::sleep(FLUSH_INTERVAL + Duration::from_millis(250)).await;
            tokio::time::timeout(Duration::from_secs(2), async {
                for _ in 0..RECORDS_WHILE_BLOCKED {
                    sender.send(RECORD).await.unwrap();
                }
            })
            .await
            .expect("socket reception stalled behind the blocked commit");

            drop(connection_guard);
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    let count: i64 = {
                        let mut connection = connection.lock().await;
                        sqlx::query_scalar("SELECT COUNT(*) FROM events")
                            .fetch_one(&mut *connection)
                            .await
                            .unwrap()
                    };
                    if count == (RECORDS_WHILE_BLOCKED + 1) as i64 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .unwrap();

            task.abort();
            let _ = task.await;
            assert!(!path.exists());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse(yaml: &str) -> Result<Vec<SocketInput>> {
        let values = serde_yaml::from_str::<Vec<SocketInputValue>>(yaml)?;
        validate_inputs(values)
    }

    #[test]
    fn plain_path_defaults_to_unix_stream() {
        let inputs = parse("- /run/evebox/eve.sock").unwrap();
        assert_eq!(inputs[0].path, Path::new("/run/evebox/eve.sock"));
        assert_eq!(inputs[0].socket_type, SocketType::UnixStream);
        assert_eq!(inputs[0].mode, None);
    }

    #[test]
    fn detailed_datagram_input_is_parsed() {
        let inputs = parse(
            r#"
- path: /run/evebox/eve.sock
  type: unix_dgram
"#,
        )
        .unwrap();
        assert_eq!(inputs[0].socket_type, SocketType::UnixDgram);
    }

    #[test]
    fn octal_socket_mode_is_parsed() {
        let inputs = parse(
            r#"
- filename: /run/evebox/eve.sock
  filetype: unix_stream
  mode: "0660"
"#,
        )
        .unwrap();
        assert_eq!(inputs[0].mode, Some(0o660));
    }

    #[test]
    fn duplicate_paths_are_rejected() {
        let err = parse(
            r#"
- /run/evebox/eve.sock
- path: /run/evebox/eve.sock
  type: unix_dgram
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("duplicate path"));
    }

    #[test]
    fn unknown_socket_type_is_rejected() {
        assert!(
            parse(
                r#"
- path: /run/evebox/eve.sock
  type: seqpacket
"#,
            )
            .is_err()
        );
    }
}
