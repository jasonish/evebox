// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! PCAP spool file discovery: rotation-sequence grouping, time-window
//! pruning, and the recursive fallback scan.

use crate::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

// Files grouped by (directory, basename, thread-id). The directory is part
// of the key as a rotation sequence never spans directories, and the pruning
// in load_files is only valid within a single rotation sequence.
pub(crate) type SortedFiles = HashMap<(PathBuf, String, u64), Vec<(u64, PathBuf)>>;

const MIN_ROTATION_TIMESTAMP: u64 = 946_684_800; // 2000-01-01 00:00:00 UTC
const MAX_ROTATION_TIMESTAMP: u64 = 4_102_444_800; // 2100-01-01 00:00:00 UTC

/// A directory of PCAP spool files to extract packets from.
#[derive(Debug, Clone)]
pub(crate) struct SpoolConfig {
    pub(crate) directory: PathBuf,
    /// If set, only files beginning with the prefix will be considered.
    pub(crate) prefix: Option<String>,
    /// File-SELECTION slack: widens the [start, end] window used to prune
    /// candidate files (not the per-packet gate). The CLI uses ZERO to
    /// preserve its historical behavior; the server will use 60s.
    pub(crate) margin: Duration,
}

impl SpoolConfig {
    pub(crate) fn new(directory: impl Into<PathBuf>, prefix: Option<String>) -> Self {
        Self {
            directory: directory.into(),
            prefix,
            margin: Duration::from_secs(60),
        }
    }
}

/// Discover spool files grouped by rotation sequence, pruned to the files
/// that could contain packets in `[start, end]` (unix microseconds, widened
/// by the spool margin).
///
/// Returns `None` if a filename that cannot be parsed as a rotated spool
/// file was seen, in which case the caller should fall back to
/// [`walk_files`].
pub(crate) fn load_files(
    spool: &SpoolConfig,
    start: u64,
    end: u64,
) -> std::io::Result<Option<SortedFiles>> {
    let margin = u64::try_from(spool.margin.as_micros()).unwrap_or(u64::MAX);
    let start_time = start.saturating_sub(margin);
    let end_time = end.saturating_add(margin);

    let mut sorted: SortedFiles = HashMap::new();
    if !collect_files(&spool.directory, spool.prefix.as_deref(), &mut sorted)? {
        return Ok(None);
    }

    let found = sorted.values().map(Vec::len).sum::<usize>();

    for files in sorted.values_mut() {
        files.sort();

        // Prune files that cannot contain packets in the requested time
        // range: a file is dropped if it was created after the end of the
        // range, or if the file after it was created before the start of the
        // range (meaning this file was rotated out before the range began).
        let tmp = files.clone();
        let mut pit = tmp.iter().peekable();
        files.retain(|e| {
            let _current = pit.next().unwrap();
            let timestamp = e.0.saturating_mul(1_000_000);
            if timestamp > end_time {
                debug!("Removing {}, it starts after the end time", e.1.display());
                return false;
            }
            if let Some(next) = pit.peek()
                && next.0.saturating_mul(1_000_000) < start_time
            {
                // The next file in our sorted list has a creating time less than our start
                // time, we can remove this file.
                debug!("Removing {}, it ends before our start time", e.1.display());
                return false;
            }
            true
        });
    }

    let remaining = sorted.values().map(Vec::len).sum::<usize>();
    info!(
        "Found {found} PCAP files, eliminated {} based on time filters",
        found - remaining
    );

    Ok(Some(sorted))
}

/// `std::fs::read_dir` with the directory named in the error, which
/// otherwise reaches the user as a bare "No such file or directory".
fn read_dir(directory: &Path) -> std::io::Result<std::fs::ReadDir> {
    std::fs::read_dir(directory)
        .map_err(|err| std::io::Error::new(err.kind(), format!("{}: {err}", directory.display())))
}

fn collect_files(
    directory: &Path,
    prefix: Option<&str>,
    sorted: &mut SortedFiles,
) -> std::io::Result<bool> {
    for entry in read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            if !collect_files(&path, prefix, sorted)? {
                return Ok(false);
            }
        } else if file_type.is_file() {
            // A filename that is not valid UTF-8 cannot match the prefix.
            if let Some(prefix) = prefix
                && !path
                    .file_name()
                    .and_then(|filename| filename.to_str())
                    .is_some_and(|filename| filename.starts_with(prefix))
            {
                continue;
            }
            let filename = path.file_name().unwrap();
            if let Some((basename, id, ts)) = parse_filename(filename) {
                sorted
                    .entry((directory.to_path_buf(), basename, id))
                    .or_default()
                    .push((ts, path));
            } else {
                // Get out of here, we can't present a fully sorted file set.
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Recursively collect all candidate files under `directory` in traversal
/// order. This is the non-optimized fallback used when the spool filenames
/// cannot be parsed for pruning. Does not follow directory symlinks.
pub(crate) fn walk_files(directory: &Path, prefix: Option<&str>) -> std::io::Result<Vec<PathBuf>> {
    let mut files = vec![];
    walk(directory, prefix, &mut files)?;
    Ok(files)
}

fn walk(directory: &Path, prefix: Option<&str>, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            walk(&path, prefix, files)?;
        } else if file_type.is_file() {
            // A filename that is not valid UTF-8 cannot match the prefix.
            if let Some(prefix) = prefix
                && !path
                    .file_name()
                    .and_then(|filename| filename.to_str())
                    .is_some_and(|filename| filename.starts_with(prefix))
            {
                continue;
            }
            files.push(path);
        } else {
            debug!("Ignoring {:?}", &path);
        }
    }
    Ok(())
}

/// Case-insensitive `str::strip_suffix` for ASCII suffixes.
fn strip_suffix_ignore_case<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
    let n = name.len().checked_sub(suffix.len())?;
    let tail = name.get(n..)?;
    if tail.eq_ignore_ascii_case(suffix) {
        Some(&name[..n])
    } else {
        None
    }
}

/// Strip one trailing capture file extension: Suricata templates like
/// `log.%n.%t.pcap` put the rotation numbers BEFORE the extension.
fn strip_capture_ext(filename: &str) -> &str {
    for ext in [".pcap", ".cap"] {
        if let Some(stripped) = strip_suffix_ignore_case(filename, ext) {
            return stripped;
        }
    }
    filename
}

fn parse_filename(filename: &OsStr) -> Option<(String, u64, u64)> {
    lazy_static! {
        // Matches the Suricata pcap-log format with thread-id.
        static ref RE_SURICATA_WITH_THREAD_ID: Regex =
            Regex::new(r"^(.*?)\.(\d+)(\.(\d+))?$").unwrap();

        // Matches a simpler format where the filename just contains a
        // timestamp.
        static ref RE_SIMPLE: Regex = Regex::new(r"(\d+)").unwrap();
    }
    if let Some(filename) = filename.to_str() {
        // Suricata's multi-mode template log.%n.%t.pcap places the
        // rotation numbers before the extension, so parse the rotation
        // fields with one trailing capture extension stripped. The
        // digit-run fallback below deliberately keeps the extension so
        // it stays part of the group key (trace-<ts>.pcap style
        // spools).
        let stripped = strip_capture_ext(filename);
        if let Some(m) = RE_SURICATA_WITH_THREAD_ID.captures(stripped) {
            let basename = m.get(1)?.as_str().to_owned();
            let a = m.get(2)?.as_str().parse::<u64>().ok()?;
            let b = if let Some(m) = m.get(4) {
                m.as_str().parse::<u64>().ok()?
            } else {
                0
            };
            let (id, ts) = if a > b { (b, a) } else { (a, b) };
            if (MIN_ROTATION_TIMESTAMP..=MAX_ROTATION_TIMESTAMP).contains(&ts) {
                return Some((basename, id, ts));
            }
            return None;
        }

        if let Some(m) = RE_SIMPLE.captures(filename) {
            let timestamp_match = m.get(1)?;
            let ts = timestamp_match.as_str().parse::<u64>().ok()?;
            if (MIN_ROTATION_TIMESTAMP..=MAX_ROTATION_TIMESTAMP).contains(&ts) {
                let basename = format!(
                    "{}{}",
                    &filename[..timestamp_match.start()],
                    &filename[timestamp_match.end()..]
                );
                return Some((basename, 0, ts));
            }
        }
    }

    None
}

#[cfg(test)]
mod test {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_parse_filename_with_thread() {
        let filename: OsString = "log.pcap.1.1649915733".into();
        let r = parse_filename(&filename).unwrap();
        assert_eq!(r, ("log.pcap".to_string(), 1, 1649915733));

        let filename: OsString = "log.pcap.11.1649915733".into();
        let r = parse_filename(&filename).unwrap();
        assert_eq!(r, ("log.pcap".to_string(), 11, 1649915733));

        let filename: OsString = "log.pcap.111.1649915733".into();
        let r = parse_filename(&filename).unwrap();
        assert_eq!(r, ("log.pcap".to_string(), 111, 1649915733));
    }

    #[test]
    fn test_parse_filename_without_thread() {
        let filename: OsString = "log.pcap.1649915733".into();
        let r = parse_filename(&filename).unwrap();
        assert_eq!(r, ("log.pcap".to_string(), 0, 1649915733));
    }

    #[test]
    fn test_parse_filename_simple_timestamp() {
        let filename: OsString = "trace-1649915733.pcap".into();
        let r = parse_filename(&filename).unwrap();
        assert_eq!(r, ("trace-.pcap".to_string(), 0, 1649915733));
    }

    #[test]
    fn test_parse_filename_extension_last() {
        // Suricata multi-mode template log.%n.%t.pcap: rotation
        // numbers before the extension.
        let r = parse_filename(&OsString::from("log.1.1649915733.pcap")).unwrap();
        assert_eq!(r, ("log".to_string(), 1, 1649915733));

        // Case-insensitive extension match.
        let r = parse_filename(&OsString::from("LOG.1.1649915733.PCAP")).unwrap();
        assert_eq!(r, ("LOG".to_string(), 1, 1649915733));

        // Other capture extensions.
        let r = parse_filename(&OsString::from("log.1.1649915733.cap")).unwrap();
        assert_eq!(r, ("log".to_string(), 1, 1649915733));
    }

    #[test]
    fn test_load_files_extension_last_sorted_path() {
        // A log.%n.%t.pcap shaped spool must take the sorted path
        // (load_files returns Some) with time-range pruning applied,
        // not the whole-directory fallback scan.
        let tempdir = tempfile::tempdir().unwrap();
        let directory = tempdir.path();

        for filename in ["log.1.1700000000.pcap", "log.1.1700001000.pcap"] {
            std::fs::File::create(directory.join(filename)).unwrap();
        }

        let spool = SpoolConfig {
            margin: Duration::ZERO,
            ..SpoolConfig::new(directory, None)
        };
        let files = load_files(&spool, 1_700_001_250_000_000, 1_700_001_500_000_000)
            .unwrap()
            .unwrap();

        let log_files = &files[&(directory.to_path_buf(), "log".to_string(), 1)];
        assert_eq!(log_files.len(), 1);
        assert_eq!(
            log_files[0].1.file_name().unwrap(),
            OsStr::new("log.1.1700001000.pcap")
        );
    }

    #[test]
    fn test_parse_filename_rejects_date_style_rotation() {
        assert!(parse_filename(&OsString::from("trace-20260701.pcap")).is_none());
        assert!(parse_filename(&OsString::from("trace-20260701120000.pcap")).is_none());
    }

    #[test]
    fn test_parse_filename_uses_trailing_rotation_fields() {
        let filename: OsString = "10.0.0.1.pcap.1.1649915733".into();
        let r = parse_filename(&filename).unwrap();
        assert_eq!(r, ("10.0.0.1.pcap".to_string(), 1, 1649915733));

        assert!(parse_filename(&OsString::from("trace.2026.07.03.pcap")).is_none());
    }

    #[test]
    fn test_load_files_groups_by_directory_basename_and_thread() {
        let tempdir = tempfile::tempdir().unwrap();
        let directory = tempdir.path();
        let first = directory.join("first");
        let second = directory.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();

        for (directory, filename) in [
            (&first, "log.pcap.1.1700001000"),
            (&second, "log.pcap.1.1700002000"),
            (&first, "log.pcap.1.1700003000"),
            (&second, "report.pcap.1.1700001200"),
        ] {
            std::fs::File::create(directory.join(filename)).unwrap();
        }

        let spool = SpoolConfig {
            margin: Duration::ZERO,
            ..SpoolConfig::new(directory, None)
        };
        let files = load_files(&spool, 0, u64::MAX).unwrap().unwrap();

        assert_eq!(files.len(), 3);
        let first_log_files = &files[&(first.clone(), "log.pcap".to_string(), 1)];
        assert_eq!(first_log_files.len(), 2);
        assert!(
            first_log_files
                .iter()
                .all(|file| file.1.parent() == Some(&first))
        );
        assert_eq!(files[&(second.clone(), "log.pcap".to_string(), 1)].len(), 1);
        assert_eq!(files[&(second, "report.pcap".to_string(), 1)].len(), 1);
    }

    #[test]
    fn test_load_files_margin_widens_selection() {
        let tempdir = tempfile::tempdir().unwrap();
        let directory = tempdir.path();

        for filename in [
            "log.pcap.1.1700001000",
            "log.pcap.1.1700002000",
            "log.pcap.1.1700003000",
        ] {
            std::fs::File::create(directory.join(filename)).unwrap();
        }

        let start = 1_700_002_100_000_000;
        let end = 1_700_002_200_000_000;
        let key = (directory.to_path_buf(), "log.pcap".to_string(), 1);

        // With no margin only the file covering the window remains.
        let spool = SpoolConfig {
            margin: Duration::ZERO,
            ..SpoolConfig::new(directory, None)
        };
        let files = load_files(&spool, start, end).unwrap().unwrap();
        assert_eq!(files[&key].len(), 1);
        assert_eq!(
            files[&key][0].1.file_name().unwrap(),
            OsStr::new("log.pcap.1.1700002000")
        );

        // A margin widens the window, keeping the preceding file as well.
        let spool = SpoolConfig {
            margin: Duration::from_secs(150),
            ..SpoolConfig::new(directory, None)
        };
        let files = load_files(&spool, start, end).unwrap().unwrap();
        assert_eq!(files[&key].len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn test_prefix_excludes_non_utf8_filenames() {
        use std::os::unix::ffi::OsStringExt;

        let tempdir = tempfile::tempdir().unwrap();
        let directory = tempdir.path();
        std::fs::File::create(directory.join("log.pcap.1.1700001000")).unwrap();
        let non_utf8 = OsString::from_vec(b"other-\xff.pcap".to_vec());
        std::fs::File::create(directory.join(&non_utf8)).unwrap();

        // The fallback scan must not include a file the prefix excludes
        // just because its name is not valid UTF-8.
        let files = walk_files(directory, Some("log.pcap")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].file_name().unwrap(),
            OsStr::new("log.pcap.1.1700001000")
        );

        // Without a prefix the fallback scan keeps every file.
        let files = walk_files(directory, None).unwrap();
        assert_eq!(files.len(), 2);

        // With the prefix filtering the foreign file out, the sorted path
        // stays available instead of aborting on its unparseable name.
        let spool = SpoolConfig::new(directory, Some("log.pcap".to_string()));
        assert!(load_files(&spool, 0, u64::MAX).unwrap().is_some());
    }
}
