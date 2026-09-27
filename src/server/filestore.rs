// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Retrieval of files extracted by Suricata's file-store (v2) output.
//!
//! File-store v2 is content addressed: a finished file lives at
//! `<dir>/<xx>/<sha256>` where `xx` is the first two hex digits of its
//! lowercase SHA-256. Files still being written live under `<dir>/tmp`
//! and are never reachable from here. The only request input that
//! reaches the filesystem is a validated 64-digit hex string, so a
//! caller can never name an arbitrary path.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::agent::protocol::CAPABILITY_FILESTORE;
use crate::prelude::*;
use crate::server::agents::AgentRegistry;
use crate::server::pcap::PcapRouting;
use crate::server::routing::{self, Resolved, RouteError};

/// A validated, lowercase SHA-256 hex digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Sha256(String);

impl Sha256 {
    /// Parse a 64-digit hex digest, accepting either case.
    pub(crate) fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.len() == 64 && input.bytes().all(|b| b.is_ascii_hexdigit()) {
            Some(Self(input.to_ascii_lowercase()))
        } else {
            None
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// The file's path relative to the file-store directory.
    #[cfg(test)]
    pub(crate) fn relative_path(&self) -> PathBuf {
        Path::new(&self.0[..2]).join(&self.0)
    }
}

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A file referenced by an event, from `fileinfo` or an alert's `files`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventFile {
    pub(crate) sha256: Sha256,
    /// The file name as seen on the wire. Attacker controlled: for
    /// display only, never used to build a path or a download name.
    pub(crate) filename: Option<String>,
    pub(crate) size: Option<u64>,
}

impl EventFile {
    fn from_fileinfo(value: &serde_json::Value) -> Option<Self> {
        let sha256 = Sha256::parse(value["sha256"].as_str()?)?;
        Some(Self {
            sha256,
            filename: value["filename"].as_str().map(str::to_string),
            size: value["size"].as_u64(),
        })
    }
}

/// The files an event references that could be in a file store, in
/// event order and deduplicated by digest.
///
/// A file-store is content addressed, so a file is retrievable whenever
/// any occurrence of the same content was stored: the per-occurrence
/// `stored` flag is deliberately ignored (an alert's `files` entries in
/// particular usually report `stored: false` because storing completes
/// after the alert is logged).
pub(crate) fn event_files(source: &serde_json::Value) -> Vec<EventFile> {
    let mut files: Vec<EventFile> = Vec::new();
    let mut push = |file: Option<EventFile>| {
        if let Some(file) = file
            && !files.iter().any(|seen| seen.sha256 == file.sha256)
        {
            files.push(file);
        }
    };
    push(EventFile::from_fileinfo(&source["fileinfo"]));
    if let Some(entries) = source["files"].as_array() {
        for entry in entries {
            push(EventFile::from_fileinfo(entry));
        }
    }
    files
}

/// Why a local file could not be served.
#[derive(Debug)]
pub(crate) enum OpenError {
    /// Not in the store: never stored, still being written, or pruned.
    NotFound,
    /// Something other than a regular file sits at the expected path.
    NotAFile,
    Io(std::io::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("file not found in the file store"),
            Self::NotAFile => f.write_str("file store entry is not a regular file"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

/// A server-local Suricata file-store directory.
#[derive(Debug, Clone)]
pub(crate) struct LocalFilestore {
    directory: PathBuf,
}

impl LocalFilestore {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    #[cfg(all(test, unix))]
    pub(crate) fn path(&self, sha256: &Sha256) -> PathBuf {
        self.directory.join(sha256.relative_path())
    }

    /// The size of a stored file, with the same checks as [`Self::open`].
    pub(crate) async fn stat(&self, sha256: &Sha256) -> Result<u64, OpenError> {
        self.open(sha256).await.map(|(_, size)| size)
    }

    /// Open a stored file for reading, returning it with its size.
    ///
    /// Nothing below the configured directory is followed: a symlink as
    /// the shard directory or as the file itself is refused, and the
    /// checks are made on the handles actually opened, so a swap between
    /// checking and opening cannot redirect the read outside the store.
    /// Suricata only ever writes plain directories and regular files here.
    pub(crate) async fn open(&self, sha256: &Sha256) -> Result<(tokio::fs::File, u64), OpenError> {
        let directory = self.directory.clone();
        let sha256 = sha256.clone();
        let (file, size) = tokio::task::spawn_blocking(move || open_in_store(&directory, &sha256))
            .await
            .map_err(|err| OpenError::Io(std::io::Error::other(err)))??;
        Ok((tokio::fs::File::from_std(file), size))
    }
}

/// Open `<directory>/<xx>/<sha256>` without following links below
/// `directory`, returning the file and its size. Blocking.
fn open_in_store(directory: &Path, sha256: &Sha256) -> Result<(std::fs::File, u64), OpenError> {
    let file = open_no_follow(directory, sha256)?;
    let metadata = file.metadata().map_err(OpenError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(OpenError::NotAFile);
    }
    Ok((file, metadata.len()))
}

/// Resolve each component relative to the handle of its parent with
/// `O_NOFOLLOW`. The file is opened non-blocking so a FIFO planted in the
/// store cannot stall the open; non-blocking mode has no effect on reads
/// of regular files, the only kind served.
#[cfg(unix)]
fn open_no_follow(directory: &Path, sha256: &Sha256) -> Result<std::fs::File, OpenError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

    fn openat(parent: RawFd, name: &str, flags: libc::c_int) -> Result<OwnedFd, OpenError> {
        let name = CString::new(name).map_err(|err| OpenError::Io(std::io::Error::other(err)))?;
        // SAFETY: `name` is a valid NUL-terminated string that outlives the
        // call, and `parent` is an open directory descriptor owned by the
        // caller for the duration of the call.
        let fd = unsafe { libc::openat(parent, name.as_ptr(), flags | libc::O_CLOEXEC) };
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            return Err(match err.raw_os_error() {
                Some(libc::ENOENT) => OpenError::NotFound,
                // ELOOP (EMLINK on FreeBSD): a symlink refused by
                // O_NOFOLLOW; ENOTDIR: the shard is not a directory.
                Some(libc::ELOOP | libc::EMLINK | libc::ENOTDIR) => OpenError::NotAFile,
                _ => OpenError::Io(err),
            });
        }
        // SAFETY: `fd` was just returned by a successful openat and is
        // owned by nobody else.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    // The configured directory itself may be a symlink: it is operator
    // controlled. Everything below it is not.
    let root = match std::fs::File::open(directory) {
        Ok(root) => root,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(OpenError::NotFound);
        }
        Err(err) => return Err(OpenError::Io(err)),
    };
    let shard = openat(
        root.as_raw_fd(),
        &sha256.as_str()[..2],
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
    )?;
    let file = openat(
        shard.as_raw_fd(),
        sha256.as_str(),
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY,
    )?;
    Ok(std::fs::File::from(file))
}

/// Windows has no `openat`; refuse a reparse point as the shard directory
/// and open the file itself as a reparse point rather than through it, so
/// a link there fails the regular-file check. A shard directory swapped
/// for a link between the check and the open is not caught here.
#[cfg(windows)]
fn open_no_follow(directory: &Path, sha256: &Sha256) -> Result<std::fs::File, OpenError> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    let shard = directory.join(&sha256.as_str()[..2]);
    match std::fs::symlink_metadata(&shard) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => return Err(OpenError::NotAFile),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(OpenError::NotFound);
        }
        Err(err) => return Err(OpenError::Io(err)),
    }
    match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(shard.join(sha256.as_str()))
    {
        Ok(file) => Ok(file),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(OpenError::NotFound),
        Err(err) => Err(OpenError::Io(err)),
    }
}

/// Extracted file retrieval: the optional server-local store.
#[derive(Default)]
pub(crate) struct FilestoreService {
    local: Option<LocalFilestore>,
}

impl FilestoreService {
    pub(crate) fn new(local: Option<LocalFilestore>) -> Self {
        Self { local }
    }

    pub(crate) fn local(&self) -> Option<&LocalFilestore> {
        self.local.as_ref()
    }

    pub(crate) fn has_local(&self) -> bool {
        self.local.is_some()
    }

    /// Resolve the source for a request across the local store and live
    /// agents advertising the `filestore` capability, using the same
    /// operator routing table and heuristics as packet capture.
    pub(crate) fn resolve_source(
        &self,
        agents: &AgentRegistry,
        routing: &PcapRouting,
        event: Option<&serde_json::Value>,
        explicit: Option<&str>,
    ) -> Result<Resolved, RouteError> {
        routing::resolve(
            agents,
            CAPABILITY_FILESTORE,
            self.has_local(),
            routing,
            event,
            explicit,
        )
    }
}

/// Build the server-local file-store service from configuration.
pub(crate) fn configure(config: &crate::config::Config) -> FilestoreService {
    let directory = config
        .get::<String>("filestore.directory")
        .unwrap_or_else(|err| {
            warn!("Ignoring bad filestore.directory: {err}; file retrieval disabled");
            None
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let local = directory.map(|directory| {
        if !directory.is_dir() {
            warn!(
                "File store directory {} does not exist (yet)",
                directory.display()
            );
        }
        info!("Serving extracted files from {}", directory.display());
        LocalFilestore::new(directory)
    });
    FilestoreService::new(local)
}

#[cfg(test)]
mod test {
    use super::*;

    const SHA: &str = "a3c5f1e2d4b6a8c0e1f3a5b7c9d0e2f4a6b8c0d1e3f5a7b9c1d2e4f6a8b0c2d4";

    #[test]
    fn sha256_parse_validates_and_lowercases() {
        let parsed = Sha256::parse(&SHA.to_ascii_uppercase()).unwrap();
        assert_eq!(parsed.as_str(), SHA);
        assert_eq!(parsed.relative_path(), Path::new("a3").join(SHA));
        assert!(Sha256::parse(&SHA[1..]).is_none());
        assert!(Sha256::parse(&format!("{}g", &SHA[1..])).is_none());
        assert!(Sha256::parse("../../../../etc/passwd").is_none());
        assert!(Sha256::parse("").is_none());
        // Multibyte input of the right byte length is still rejected.
        assert!(Sha256::parse(&"é".repeat(32)).is_none());
    }

    #[test]
    fn event_files_reads_fileinfo_and_alert_files_deduplicated() {
        let other = "b".repeat(64);
        let event = serde_json::json!({
            "event_type": "alert",
            "fileinfo": { "filename": "/a.exe", "sha256": SHA, "size": 42, "stored": true },
            "files": [
                { "filename": "/a.exe", "sha256": SHA.to_ascii_uppercase(), "stored": false },
                { "filename": "/b.bin", "sha256": other, "size": 7 },
                { "filename": "/no-hash.bin" },
                { "filename": "/bad-hash.bin", "sha256": "xyz" },
            ],
        });
        let files = event_files(&event);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].sha256.as_str(), SHA);
        assert_eq!(files[0].filename.as_deref(), Some("/a.exe"));
        assert_eq!(files[0].size, Some(42));
        assert_eq!(files[1].sha256.as_str(), other);
        assert!(event_files(&serde_json::json!({"event_type": "flow"})).is_empty());
    }

    fn store_with_file(dir: &Path, content: &[u8]) -> LocalFilestore {
        let sha = Sha256::parse(SHA).unwrap();
        std::fs::create_dir_all(dir.join(&SHA[..2])).unwrap();
        std::fs::write(dir.join(sha.relative_path()), content).unwrap();
        LocalFilestore::new(dir.to_path_buf())
    }

    #[tokio::test]
    async fn local_store_opens_regular_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_with_file(dir.path(), b"hello");
        let sha = Sha256::parse(SHA).unwrap();
        assert_eq!(store.stat(&sha).await.unwrap(), 5);
        let (_, size) = store.open(&sha).await.unwrap();
        assert_eq!(size, 5);

        let missing = Sha256::parse(&"c".repeat(64)).unwrap();
        assert!(matches!(
            store.open(&missing).await,
            Err(OpenError::NotFound)
        ));

        let dir_entry = Sha256::parse(&"d".repeat(64)).unwrap();
        std::fs::create_dir_all(dir.path().join(dir_entry.relative_path())).unwrap();
        assert!(matches!(
            store.open(&dir_entry).await,
            Err(OpenError::NotAFile)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_store_refuses_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_with_file(dir.path(), b"hello");
        let target = dir.path().join("secret");
        std::fs::write(&target, b"secret").unwrap();
        let link = Sha256::parse(&"e".repeat(64)).unwrap();
        std::fs::create_dir_all(dir.path().join("ee")).unwrap();
        std::os::unix::fs::symlink(&target, store.path(&link)).unwrap();
        assert!(matches!(store.open(&link).await, Err(OpenError::NotAFile)));
        assert!(matches!(store.stat(&link).await, Err(OpenError::NotAFile)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_store_refuses_a_symlinked_shard_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store_dir = dir.path().join("store");
        std::fs::create_dir_all(&store_dir).unwrap();
        let store = LocalFilestore::new(store_dir.clone());
        // A directory outside the store holding a file named like a digest.
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let sha = Sha256::parse(&"f".repeat(64)).unwrap();
        std::fs::write(outside.join(sha.as_str()), b"secret").unwrap();
        std::os::unix::fs::symlink(&outside, store_dir.join("ff")).unwrap();
        assert!(matches!(store.open(&sha).await, Err(OpenError::NotAFile)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_store_refuses_a_fifo_without_blocking() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_with_file(dir.path(), b"hello");
        let fifo = Sha256::parse(&"a".repeat(64)).unwrap();
        std::fs::create_dir_all(dir.path().join("aa")).unwrap();
        let status = std::process::Command::new("mkfifo")
            .arg(store.path(&fifo))
            .status()
            .unwrap();
        assert!(status.success());
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), store.open(&fifo))
            .await
            .expect("opening a FIFO must not block");
        assert!(matches!(result, Err(OpenError::NotAFile)));
    }
}
