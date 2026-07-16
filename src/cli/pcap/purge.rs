// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Ported from "dumpy purge".

use crate::prelude::*;
use clap::{ArgGroup, Parser as ClapParser};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{error, info, warn};

#[derive(Debug, ClapParser)]
#[command(group(
    ArgGroup::new("retention")
        .required(true)
        .args(["keep_files", "max_size"])
))]
pub(super) struct PurgeArgs {
    /// Directory containing PCAP files to purge
    directory: PathBuf,

    /// Keep only the newest N files
    #[arg(long)]
    keep_files: Option<usize>,

    /// Keep the newest files up to this total size (for example, 10G or 500M)
    #[arg(long, value_name = "SIZE", value_parser = parse_size)]
    max_size: Option<u64>,

    /// Only process files with this prefix
    #[arg(long)]
    prefix: Option<String>,

    /// Actually delete files (the default is a dry run)
    #[arg(long, short)]
    force: bool,

    /// Run purge every N minutes
    #[arg(long, short)]
    interval: Option<NonZeroU64>,

    /// Only log warnings and errors
    #[arg(long, short)]
    quiet: bool,
}

#[derive(Debug)]
struct FileInfo {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

pub(super) async fn main(args: PurgeArgs) -> Result<()> {
    if let Some(interval_minutes) = args.interval {
        if !args.quiet {
            info!(
                "Starting purge with {}-minute interval",
                interval_minutes.get()
            );
        }
        let interval = Duration::from_secs(interval_minutes.get().saturating_mul(60));
        loop {
            if let Err(err) = run_purge_once(&args) {
                error!("Purge failed: {err}");
            }

            if !args.quiet {
                info!("Sleeping for {} minutes", interval_minutes.get());
            }
            tokio::time::sleep(interval).await;
        }
    } else {
        run_purge_once(&args)
    }
}

fn run_purge_once(args: &PurgeArgs) -> Result<()> {
    let files = collect_pcap_files(&args.directory, args.prefix.as_deref())?;
    if files.is_empty() {
        if !args.quiet {
            info!(
                "No PCAP files found in directory '{}'",
                args.directory.display()
            );
        }
        return Ok(());
    }

    let files_to_delete = if let Some(keep_count) = args.keep_files {
        calculate_files_to_delete_by_count(&files, keep_count)
    } else if let Some(max_size) = args.max_size {
        calculate_files_to_delete_by_size(&files, max_size)
    } else {
        bail!("Either --keep-files or --max-size must be specified");
    };

    if files_to_delete.is_empty() {
        if !args.quiet {
            info!("No files need to be deleted");
        }
        return Ok(());
    }

    let total_size = files_to_delete
        .iter()
        .fold(0u64, |total, file| total.saturating_add(file.size));
    let total_size_mb = total_size as f64 / 1024.0 / 1024.0;

    if !args.force {
        if !args.quiet {
            info!(
                "Would delete {} files ({total_size_mb:.2} MB)",
                files_to_delete.len()
            );
            for file in &files_to_delete {
                info!("  {}", file.path.display());
            }
        }
        warn!("To actually delete these files, run with --force");
        return Ok(());
    }

    if !args.quiet {
        info!(
            "Deleting {} files ({total_size_mb:.2} MB)",
            files_to_delete.len()
        );
    }

    delete_files(&files_to_delete, args.quiet)
}

fn delete_files(files: &[&FileInfo], quiet: bool) -> Result<()> {
    let mut delete_count = 0;
    let mut delete_errors = 0;
    for file in files {
        match std::fs::remove_file(&file.path) {
            Ok(()) => {
                delete_count += 1;
                if !quiet {
                    info!("Deleted: {}", file.path.display());
                }
            }
            Err(err) => {
                delete_errors += 1;
                error!("Failed to delete {}: {err}", file.path.display());
            }
        }
    }

    if !quiet {
        info!("Deleted {delete_count} files successfully, {delete_errors} errors");
    }
    if delete_errors > 0 {
        bail!(
            "failed to delete {delete_errors} of {} capture files",
            files.len()
        );
    }
    Ok(())
}

fn collect_pcap_files(directory: &Path, prefix: Option<&str>) -> Result<Vec<FileInfo>> {
    if !directory.exists() {
        bail!("Directory does not exist: {}", directory.display());
    }
    if !directory.is_dir() {
        bail!("Path is not a directory: {}", directory.display());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|filename| filename.to_str()) else {
            continue;
        };
        if !is_pcap_filename(filename) {
            continue;
        }
        if let Some(prefix) = prefix
            && !filename.starts_with(prefix)
        {
            continue;
        }

        let metadata = entry.metadata()?;
        files.push(FileInfo {
            path,
            size: metadata.len(),
            modified: metadata.modified()?,
        });
    }

    files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(files)
}

fn is_pcap_filename(filename: &str) -> bool {
    let lowercase = filename.to_ascii_lowercase();

    [".pcap", ".cap"].iter().any(|extension| {
        lowercase.ends_with(extension)
            || lowercase
                .split_once(&format!("{extension}."))
                .is_some_and(|(_, suffix)| {
                    !suffix.is_empty()
                        && suffix
                            .split('.')
                            .all(|component| component.chars().all(|c| c.is_ascii_digit()))
                })
    })
}

fn calculate_files_to_delete_by_count(files: &[FileInfo], keep_count: usize) -> Vec<&FileInfo> {
    files.get(keep_count..).unwrap_or_default().iter().collect()
}

fn calculate_files_to_delete_by_size(files: &[FileInfo], max_size_bytes: u64) -> Vec<&FileInfo> {
    let mut total_size = 0u64;
    let mut keep_count = 0;

    for file in files {
        let next_size = total_size.saturating_add(file.size);
        if next_size > max_size_bytes {
            break;
        }
        total_size = next_size;
        keep_count += 1;
    }

    files[keep_count..].iter().collect()
}

fn parse_size(input: &str) -> std::result::Result<u64, String> {
    let size = input.trim().to_ascii_uppercase();
    if size.is_empty() {
        return Err("size cannot be empty".to_string());
    }

    let (number, multiplier) = match size.as_bytes().last().copied() {
        Some(b'K') => (&size[..size.len() - 1], 1024f64),
        Some(b'M') => (&size[..size.len() - 1], 1024f64.powi(2)),
        Some(b'G') => (&size[..size.len() - 1], 1024f64.powi(3)),
        Some(b'T') => (&size[..size.len() - 1], 1024f64.powi(4)),
        _ => {
            return size
                .parse::<u64>()
                .map_err(|_| format!("invalid size '{input}'"));
        }
    };

    let number = number
        .parse::<f64>()
        .map_err(|_| format!("invalid size '{input}'"))?;
    let bytes = number * multiplier;
    if !bytes.is_finite() || bytes < 0.0 || bytes >= u64::MAX as f64 {
        return Err(format!("size '{input}' is out of range"));
    }
    Ok(bytes as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;

    fn purge_args(directory: &Path) -> PurgeArgs {
        PurgeArgs {
            directory: directory.to_path_buf(),
            keep_files: Some(0),
            max_size: None,
            prefix: None,
            force: false,
            interval: None,
            quiet: true,
        }
    }

    fn write_file(path: &Path, size: usize, modified: u64) {
        std::fs::write(path, vec![0; size]).unwrap();
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_times(
            FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(modified)),
        )
        .unwrap();
    }

    #[test]
    fn parse_sizes() {
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert_eq!(parse_size("1K").unwrap(), 1024);
        assert_eq!(parse_size("1m").unwrap(), 1024 * 1024);
        assert_eq!(parse_size("1.5G").unwrap(), 1_610_612_736);
        assert_eq!(parse_size("2T").unwrap(), 2 * 1024 * 1024 * 1024 * 1024);
        assert!(parse_size("").is_err());
        assert!(parse_size("-1M").is_err());
        assert!(parse_size("NaNG").is_err());
        assert!(parse_size("10MB").is_err());
    }

    #[test]
    fn cli_requires_one_retention_mode() {
        let directory = "/tmp/captures";
        assert!(PurgeArgs::try_parse_from(["purge", directory]).is_err());
        assert!(
            PurgeArgs::try_parse_from([
                "purge",
                directory,
                "--keep-files",
                "1",
                "--max-size",
                "1G",
            ])
            .is_err()
        );
        assert!(
            PurgeArgs::try_parse_from(
                ["purge", directory, "--keep-files", "1", "--interval", "0",]
            )
            .is_err()
        );
    }

    #[test]
    fn cli_accepts_dumpy_compose_argument_order() {
        let args = PurgeArgs::try_parse_from([
            "purge",
            "--prefix",
            "dump-",
            "--max-size",
            "500G",
            "-i",
            "1",
            "/mnt/data/capture",
            "--force",
            "--quiet",
        ])
        .unwrap();

        assert_eq!(args.directory, Path::new("/mnt/data/capture"));
        assert_eq!(args.prefix.as_deref(), Some("dump-"));
        assert_eq!(args.max_size, Some(500 * 1024 * 1024 * 1024));
        assert_eq!(args.interval.unwrap().get(), 1);
        assert!(args.force);
        assert!(args.quiet);
    }

    #[test]
    fn capture_filename_detection_covers_supported_spool_names() {
        for filename in ["capture.pcap", "log.pcap.1.1700000000"] {
            assert!(is_pcap_filename(filename), "{filename}");
        }
        for filename in [
            "README.txt",
            "capture.pcap.backup",
            "capture.cap.compressed",
            "log.1.1700000000.pcap.compressed",
            "log.pcap.current",
        ] {
            assert!(!is_pcap_filename(filename), "{filename}");
        }
    }

    #[test]
    fn dry_run_preserves_files_and_force_deletes_only_old_files() {
        let tempdir = tempfile::tempdir().unwrap();
        let oldest = tempdir.path().join("log.1.1.pcap");
        let middle = tempdir.path().join("log.1.2.pcap");
        let newest = tempdir.path().join("log.1.3.pcap");
        write_file(&oldest, 10, 1);
        write_file(&middle, 10, 2);
        write_file(&newest, 10, 3);

        let mut args = purge_args(tempdir.path());
        args.keep_files = Some(1);
        run_purge_once(&args).unwrap();
        assert!(oldest.exists());
        assert!(middle.exists());
        assert!(newest.exists());

        args.force = true;
        run_purge_once(&args).unwrap();
        assert!(!oldest.exists());
        assert!(!middle.exists());
        assert!(newest.exists());
    }

    #[test]
    fn deletion_failures_are_returned_after_other_files_are_attempted() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("capture.pcap");
        write_file(&path, 10, 1);
        let file = FileInfo {
            path: path.clone(),
            size: 10,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        };

        // The first entry is deleted; the duplicate then fails with NotFound.
        // This is deterministic even when the tests run as root.
        let err = delete_files(&[&file, &file], true).unwrap_err();

        assert!(!path.exists());
        assert!(
            err.to_string()
                .contains("failed to delete 1 of 2 capture files")
        );
    }

    #[test]
    fn prefix_and_file_type_filters_limit_deletion() {
        let tempdir = tempfile::tempdir().unwrap();
        let matching = tempdir.path().join("log.1.1.pcap");
        let other_prefix = tempdir.path().join("other.1.1.pcap");
        let non_capture = tempdir.path().join("log.notes.txt");
        let nested_dir = tempdir.path().join("nested");
        let nested = nested_dir.join("log.1.2.pcap");
        std::fs::create_dir(&nested_dir).unwrap();
        write_file(&matching, 10, 1);
        write_file(&other_prefix, 10, 1);
        write_file(&non_capture, 10, 1);
        write_file(&nested, 10, 1);

        let mut args = purge_args(tempdir.path());
        args.prefix = Some("log.".to_string());
        args.force = true;
        run_purge_once(&args).unwrap();

        assert!(!matching.exists());
        assert!(other_prefix.exists());
        assert!(non_capture.exists());
        assert!(nested.exists());
    }

    #[test]
    fn max_size_keeps_newest_files_that_fit() {
        let files = [
            FileInfo {
                path: "newest.pcap".into(),
                size: 60,
                modified: SystemTime::UNIX_EPOCH + Duration::from_secs(3),
            },
            FileInfo {
                path: "middle.pcap".into(),
                size: 40,
                modified: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
            },
            FileInfo {
                path: "oldest.pcap".into(),
                size: 20,
                modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            },
        ];

        let deleted = calculate_files_to_delete_by_size(&files, 100);
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].path, Path::new("oldest.pcap"));

        let deleted = calculate_files_to_delete_by_size(&files, 59);
        assert_eq!(deleted.len(), 3);
    }
}
