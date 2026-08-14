// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Discovery of timestamped EVE spool files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const TIMESTAMP_LENGTH: usize = 10;

/// One physical file in an EVE spool.
///
/// The final dot-separated component is a Unix timestamp. Everything before
/// it identifies the stream, so threaded files naturally form independent
/// streams (`eve.json.1.`, `eve.json.2.`, and so on).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpoolFile {
    path: PathBuf,
    key: PathBuf,
    timestamp: u64,
}

impl SpoolFile {
    pub(crate) fn parse(path: PathBuf) -> Option<Self> {
        let filename = path.file_name()?.to_str()?;
        let (prefix, timestamp) = filename.rsplit_once('.')?;
        if prefix.is_empty()
            || timestamp.len() != TIMESTAMP_LENGTH
            || !timestamp.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }

        let timestamp = timestamp.parse().ok()?;
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let key = directory.join(format!("{prefix}."));
        let path = directory.join(filename);

        Some(Self {
            path,
            key,
            timestamp,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn key(&self) -> &Path {
        &self.key
    }

    pub(crate) fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Find the oldest file in this stream newer than this file.
    pub(crate) fn next(&self) -> std::io::Result<Option<Self>> {
        self.newer_file(false)
    }

    /// Find the newest file in this stream newer than this file.
    pub(crate) fn latest(&self) -> std::io::Result<Option<Self>> {
        self.newer_file(true)
    }

    fn newer_file(&self, latest: bool) -> std::io::Result<Option<Self>> {
        let directory = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut next = None;

        for entry in std::fs::read_dir(directory)? {
            let Some(candidate) = Self::parse(entry?.path()) else {
                continue;
            };
            if candidate.key != self.key || candidate.timestamp <= self.timestamp {
                continue;
            }
            if next.as_ref().is_none_or(|current: &Self| {
                if latest {
                    candidate.timestamp > current.timestamp
                } else {
                    candidate.timestamp < current.timestamp
                }
            }) {
                next = Some(candidate);
            }
        }

        Ok(next)
    }
}

/// A configured EVE input after physical spool files have been grouped into
/// logical streams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EveInput {
    File(PathBuf),
    Spool(SpoolFile),
}

impl EveInput {
    fn from_path(path: PathBuf) -> Self {
        match SpoolFile::parse(path.clone()) {
            Some(file) => Self::Spool(file),
            None => Self::File(path),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::File(path) => path,
            Self::Spool(file) => file.path(),
        }
    }

    /// Stable identity for this input. For a spool this excludes the changing
    /// timestamp but retains the thread index.
    pub(crate) fn key(&self) -> &Path {
        match self {
            Self::File(path) => path,
            Self::Spool(file) => file.key(),
        }
    }

    pub(crate) fn spool_file(&self) -> Option<&SpoolFile> {
        match self {
            Self::Spool(file) => Some(file),
            Self::File(_) => None,
        }
    }

    pub(crate) fn is_spool(&self) -> bool {
        matches!(self, Self::Spool(_))
    }

    /// Collapse physical paths into logical inputs. The oldest spool file is
    /// selected for normal startup so a backlog is read in order. Tail mode
    /// starts with the newest file.
    pub(crate) fn group(paths: impl IntoIterator<Item = PathBuf>, end: bool) -> Vec<Self> {
        let mut inputs = BTreeMap::new();

        for path in paths {
            let candidate = Self::from_path(path);
            let key = candidate.key().to_path_buf();
            match inputs.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(candidate);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let (Self::Spool(current), Self::Spool(candidate_file)) =
                        (entry.get(), &candidate)
                    else {
                        continue;
                    };
                    let replace = if end {
                        candidate_file.timestamp() > current.timestamp()
                    } else {
                        candidate_file.timestamp() < current.timestamp()
                    };
                    if replace {
                        entry.insert(candidate);
                    }
                }
            }
        }

        inputs.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamped_and_threaded_spool_files() {
        let file = SpoolFile::parse(PathBuf::from("/logs/eve.json.1700000000")).unwrap();
        assert_eq!(file.key(), Path::new("/logs/eve.json."));
        assert_eq!(file.timestamp(), 1_700_000_000);

        let file = SpoolFile::parse(PathBuf::from("/logs/eve.json.12.1700000001")).unwrap();
        assert_eq!(file.key(), Path::new("/logs/eve.json.12."));
        assert_eq!(file.timestamp(), 1_700_000_001);
    }

    #[test]
    fn rejects_non_spool_names() {
        for path in [
            "eve.json",
            "eve.json.1",
            "eve.json.170000000",
            "eve.json.17000000000",
            "eve.json.not-a-timestamp",
        ] {
            assert!(SpoolFile::parse(PathBuf::from(path)).is_none(), "{path}");
        }
    }

    #[test]
    fn groups_each_thread_and_selects_the_requested_end() {
        let paths = [
            "/logs/eve.json.1.1700000002",
            "/logs/eve.json.1.1700000001",
            "/logs/eve.json.2.1700000001",
            "/logs/eve.json.1700000001",
            "/logs/plain.json",
        ]
        .map(PathBuf::from);

        let inputs = EveInput::group(paths.clone(), false);
        assert_eq!(inputs.len(), 4);
        let thread_one = inputs
            .iter()
            .find(|input| input.key() == Path::new("/logs/eve.json.1."))
            .unwrap();
        assert_eq!(thread_one.path(), Path::new("/logs/eve.json.1.1700000001"));

        let inputs = EveInput::group(paths, true);
        let thread_one = inputs
            .iter()
            .find(|input| input.key() == Path::new("/logs/eve.json.1."))
            .unwrap();
        assert_eq!(thread_one.path(), Path::new("/logs/eve.json.1.1700000002"));
    }

    #[test]
    fn finds_the_next_file_in_only_the_same_stream() {
        let dir = tempfile::tempdir().unwrap();
        for filename in [
            "eve.json.1.1700000001",
            "eve.json.1.1700000003",
            "eve.json.1.1700000002",
            "eve.json.2.1700000002",
            "eve.json.1700000002",
        ] {
            std::fs::File::create(dir.path().join(filename)).unwrap();
        }

        let current = SpoolFile::parse(dir.path().join("eve.json.1.1700000001")).unwrap();
        let next = current.next().unwrap().unwrap();
        assert_eq!(next.path(), dir.path().join("eve.json.1.1700000002"));
    }
}
