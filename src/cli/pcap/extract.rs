// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::pcap::{
    FetchError, FetchStats, Limits, PcapFilter, PcapRequest, PcapSource, SpoolConfig,
};
use crate::prelude::*;
use chrono::TimeZone;
use clap::Parser as ClapParser;
use same_file::Handle;
use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug, ClapParser)]
pub(super) struct ExtractArgs {
    /// Directory to process
    #[clap(long)]
    directory: String,

    /// Optional PCAP filter
    #[clap(long)]
    filter: Option<String>,

    /// Filename prefix
    ///
    /// If set, only files beginning with the provided prefix will be processed.
    #[clap(long)]
    prefix: Option<String>,

    /// Start time (YYYYMMDD or YYYYMMDDTHH:MM:SS[.ffffff][TZ])
    ///
    /// If provided, packets with a timestamp older than the one provided will
    /// not be extracted. Times without a timezone use the local timezone.
    #[clap(long, value_parser = parse_start_time)]
    start_time: Option<u64>,

    /// Duration in seconds after start-time
    ///
    /// Sets the duration in seconds after start-time that packets will be
    /// considered for extraction. This refers to the packet timestamp, not real
    /// time.
    #[clap(long, requires = "start_time")]
    duration: Option<u64>,

    /// Output filename
    ///
    /// Defaults to stdout. Output to a terminal is refused.
    #[clap(long)]
    output: Option<String>,
}

pub(super) fn main(args: ExtractArgs) -> anyhow::Result<()> {
    validate_output(&args, std::io::stdout().is_terminal())?;

    let end = match args.duration {
        Some(_) => Some(end_time(&args)?),
        None => None,
    };

    let spool = SpoolConfig {
        // A zero margin preserves this command's historical file pruning.
        margin: Duration::ZERO,
        ..SpoolConfig::new(&args.directory, args.prefix.clone())
    };
    let request = PcapRequest {
        filter: args.filter.clone().map(PcapFilter::Expression),
        start: args.start_time,
        end,
        limits: Limits::default(),
    };

    let mut out = LazyOutput::new(output_filename(&args));
    let cancel = CancellationToken::new();
    let source = PcapSource::Spool(spool);
    match crate::pcap::fetch(&source, &request, &mut out, &cancel) {
        Ok(_stats) => {
            out.flush()?;
            Ok(())
        }
        Err(FetchError::NoCandidateFiles) => write_empty_output(&args, FetchStats::default()),
        Err(FetchError::NoMatch(stats)) => write_empty_output(&args, stats),
        Err(err) => Err(err.into()),
    }
}

/// Write an empty, header-only capture to the output, using the link type
/// of the first spool file that was opened, if any.
fn write_empty_output(args: &ExtractArgs, stats: FetchStats) -> Result<()> {
    let dead = pcap::Capture::dead(stats.linktype.unwrap_or(pcap::Linktype::ETHERNET))?;
    let mut savefile = dead.savefile(output_filename(args))?;
    savefile.flush()?;
    info!("No matching packets found, wrote an empty output capture");
    Ok(())
}

/// The output stream, opened only when the first byte is written so a
/// failed extraction does not create or clobber the output file (a no-match
/// extraction still writes an empty capture, see write_empty_output).
struct LazyOutput {
    filename: String,
    out: Option<Box<dyn Write>>,
}

impl LazyOutput {
    fn new(filename: &str) -> Self {
        Self {
            filename: filename.to_string(),
            out: None,
        }
    }
}

impl Write for LazyOutput {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.out.is_none() {
            let out: Box<dyn Write> = if self.filename == "-" {
                Box::new(std::io::stdout().lock())
            } else {
                Box::new(std::io::BufWriter::new(std::fs::File::create(
                    &self.filename,
                )?))
            };
            self.out = Some(out);
        }
        self.out.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.out {
            Some(out) => out.flush(),
            None => Ok(()),
        }
    }
}

fn output_filename(args: &ExtractArgs) -> &str {
    args.output.as_deref().unwrap_or("-")
}

fn validate_output(args: &ExtractArgs, stdout_is_terminal: bool) -> Result<()> {
    let filename = output_filename(args);
    if filename == "-" {
        if stdout_is_terminal {
            bail!("refusing to write PCAP data to a terminal; pipe stdout or use --output <FILE>");
        }
        return Ok(());
    }

    // Fetch discovers every input before LazyOutput creates a new file, so a
    // genuinely new output inside the spool is safe. For an existing output,
    // compare file identity against every recursively selected input so path,
    // symlink, and hard-link aliases are all rejected before truncation.
    let output = match file_identity(Path::new(filename)) {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    for input in crate::pcap::walk_files(Path::new(&args.directory), args.prefix.as_deref())? {
        let input_identity = match file_identity(&input) {
            Ok(identity) => identity,
            // Rotation can remove a discovered input before it is opened;
            // fetch treats that as a vanished file too.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err.into()),
        };
        if output == input_identity {
            bail!(
                "refusing to overwrite output {filename}: it aliases input {}",
                input.display()
            );
        }
    }
    Ok(())
}

/// Open a stable identity handle without creating or truncating the path.
/// Read access normally suffices; the write-only fallback still lets an
/// existing output be compared safely before LazyOutput opens it.
fn file_identity(path: &Path) -> std::io::Result<Handle> {
    let file = match std::fs::OpenOptions::new().read(true).open(path) {
        Ok(file) => file,
        Err(read_err) => std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|_| read_err)?,
    };
    Handle::from_file(file)
}

fn end_time(args: &ExtractArgs) -> Result<u64> {
    match args.duration {
        Some(duration) => duration
            .checked_mul(1_000_000)
            .and_then(|duration| args.start_time.unwrap_or(0).checked_add(duration))
            .ok_or_else(|| anyhow::anyhow!("start time plus duration is too large")),
        None => Ok(u64::MAX),
    }
}

fn parse_start_time(input: &str) -> std::result::Result<u64, String> {
    let datetime =
        if let Ok(datetime) = chrono::DateTime::parse_from_str(input, "%Y%m%dT%H:%M:%S%.f%#z") {
            datetime.fixed_offset()
        } else {
            let naive = chrono::NaiveDateTime::parse_from_str(input, "%Y%m%dT%H:%M:%S%.f")
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(input, "%Y%m%d")
                        .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
                })
                .map_err(|_| "expected YYYYMMDD or YYYYMMDDTHH:MM:SS[.ffffff][TZ]".to_string())?;
            chrono::Local
                .from_local_datetime(&naive)
                .single()
                .ok_or_else(|| "time is ambiguous or invalid in the local timezone".to_string())?
                .fixed_offset()
        };

    u64::try_from(datetime.timestamp_micros())
        .map_err(|_| "start time must be after the Unix epoch".to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pcap::testutil::{count_packets, write_pcap_file, write_pcap_file_with_linktype};

    fn extract_args(input_dir: &Path, output: &Path) -> ExtractArgs {
        ExtractArgs {
            directory: input_dir.to_string_lossy().into_owned(),
            filter: None,
            prefix: None,
            start_time: None,
            duration: None,
            output: Some(output.to_string_lossy().into_owned()),
        }
    }

    #[test]
    fn test_extract_with_libpcap_filter() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = tempdir.path().join("filtered.pcap");

        write_pcap_file(
            &input_dir.join("packets.pcap"),
            &[
                (1_700_000_000, 53),
                (1_700_000_000, 9999),
                (1_700_000_001, 53),
            ],
        );

        main(ExtractArgs {
            filter: Some("udp and port 53".to_string()),
            start_time: Some(1_699_999_999_000_000),
            duration: Some(1),
            ..extract_args(&input_dir, &output)
        })
        .unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_extract_sorted_spool_by_time_range() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = tempdir.path().join("extracted.pcap");

        write_pcap_file(
            &input_dir.join("log.pcap.1.1700000000"),
            &[(1_700_000_000, 53), (1_700_000_500, 53)],
        );
        write_pcap_file(
            &input_dir.join("log.pcap.1.1700001000"),
            &[(1_700_001_000, 53)],
        );

        main(ExtractArgs {
            start_time: Some(1_700_000_400_000_000),
            duration: Some(200),
            ..extract_args(&input_dir, &output)
        })
        .unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_extract_with_prefix() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = tempdir.path().join("extracted.pcap");

        write_pcap_file(
            &input_dir.join("log.pcap.1.1700000000"),
            &[(1_700_000_000, 53)],
        );
        write_pcap_file(
            &input_dir.join("other.pcap.1.1700000000"),
            &[(1_700_000_000, 53), (1_700_000_001, 53)],
        );

        main(ExtractArgs {
            prefix: Some("log.".to_string()),
            ..extract_args(&input_dir, &output)
        })
        .unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_extract_recursive() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        let nested = input_dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let output = tempdir.path().join("extracted.pcap");

        write_pcap_file(&nested.join("packets.pcap"), &[(1_700_000_000, 53)]);

        main(extract_args(&input_dir, &output)).unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_extract_skips_mismatched_linktypes() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = tempdir.path().join("extracted.pcap");

        write_pcap_file(&input_dir.join("a.pcap.1700000000"), &[(1_700_000_000, 53)]);
        write_pcap_file_with_linktype(
            &input_dir.join("b.pcap.1700000000"),
            &[(1_700_000_000, 53)],
            pcap::Linktype::NULL,
        );

        main(extract_args(&input_dir, &output)).unwrap();

        let capture = pcap::Capture::from_file(&output).unwrap();
        assert_eq!(capture.get_datalink(), pcap::Linktype::ETHERNET);
        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_extract_skips_unreadable_files() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = tempdir.path().join("extracted.pcap");

        std::fs::write(input_dir.join("README.txt"), "not a pcap file").unwrap();
        write_pcap_file(&input_dir.join("packets.pcap"), &[(1_700_000_000, 53)]);

        main(extract_args(&input_dir, &output)).unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_extract_writes_empty_capture_when_no_packets_match() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = tempdir.path().join("extracted.pcap");

        write_pcap_file(&input_dir.join("packets.pcap"), &[(1_700_000_000, 53)]);
        write_pcap_file(&output, &[(1_600_000_000, 53), (1_600_000_001, 53)]);

        main(ExtractArgs {
            filter: Some("udp and port 9999".to_string()),
            ..extract_args(&input_dir, &output)
        })
        .unwrap();

        let capture = pcap::Capture::from_file(&output).unwrap();
        assert_eq!(capture.get_datalink(), pcap::Linktype::ETHERNET);
        assert_eq!(count_packets(&output), 0);
    }

    #[cfg(unix)]
    #[test]
    fn test_extract_does_not_follow_directory_symlinks() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = tempdir.path().join("extracted.pcap");

        write_pcap_file(&input_dir.join("packets.pcap"), &[(1_700_000_000, 53)]);
        std::os::unix::fs::symlink(&input_dir, input_dir.join("loop")).unwrap();

        main(extract_args(&input_dir, &output)).unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_duration_requires_start_time() {
        let result = ExtractArgs::try_parse_from([
            "extract",
            "--directory",
            "input",
            "--duration",
            "60",
            "--output",
            "output.pcap",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_times_and_overflow_are_rejected() {
        let invalid_start = ExtractArgs::try_parse_from([
            "extract",
            "--directory",
            "input",
            "--start-time=-1",
            "--output",
            "output.pcap",
        ]);
        assert!(invalid_start.is_err());

        let negative_duration = ExtractArgs::try_parse_from([
            "extract",
            "--directory",
            "input",
            "--start-time",
            "20240101",
            "--duration=-1",
            "--output",
            "output.pcap",
        ]);
        assert!(negative_duration.is_err());

        let args = ExtractArgs {
            directory: "input".to_string(),
            filter: None,
            prefix: None,
            start_time: Some(u64::MAX),
            duration: Some(1),
            output: Some("output.pcap".to_string()),
        };
        assert!(end_time(&args).is_err());
    }

    #[test]
    fn test_existing_output_inside_input_is_rejected_without_truncation() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let output = input_dir.join("capture.pcap");
        write_pcap_file(&output, &[(1_700_000_000, 53), (1_700_000_001, 53)]);
        let before = std::fs::read(&output).unwrap();

        let err = main(extract_args(&input_dir, &output)).unwrap_err();
        assert!(err.to_string().contains("aliases input"));
        assert_eq!(std::fs::read(&output).unwrap(), before);
    }

    #[test]
    fn test_external_hard_link_output_is_rejected_without_truncation() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        let input = input_dir.join("capture.pcap");
        write_pcap_file(&input, &[(1_700_000_000, 53), (1_700_000_001, 53)]);
        let output = tempdir.path().join("external-output.pcap");
        std::fs::hard_link(&input, &output).unwrap();
        let before = std::fs::read(&input).unwrap();

        let err = main(extract_args(&input_dir, &output)).unwrap_err();
        assert!(err.to_string().contains("aliases input"));
        assert_eq!(std::fs::read(&input).unwrap(), before);
        assert_eq!(std::fs::read(&output).unwrap(), before);
    }

    #[test]
    fn test_existing_output_excluded_by_prefix_is_allowed() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        write_pcap_file(
            &input_dir.join("log.pcap.1.1700000000"),
            &[(1_700_000_000, 53)],
        );
        let output = input_dir.join("result.pcap");
        write_pcap_file(&output, &[(1_600_000_000, 53), (1_600_000_001, 53)]);

        main(ExtractArgs {
            prefix: Some("log.".to_string()),
            ..extract_args(&input_dir, &output)
        })
        .unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_new_output_inside_input_is_safe() {
        let tempdir = tempfile::tempdir().unwrap();
        let input_dir = tempdir.path().join("input");
        std::fs::create_dir(&input_dir).unwrap();
        write_pcap_file(&input_dir.join("capture.pcap"), &[(1_700_000_000, 53)]);
        let output = input_dir.join("filtered.pcap");

        main(extract_args(&input_dir, &output)).unwrap();

        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_output_defaults_to_non_terminal_stdout() {
        let args = ExtractArgs::try_parse_from(["extract", "--directory", "input"]).unwrap();
        assert_eq!(output_filename(&args), "-");
        assert!(validate_output(&args, false).is_ok());
        assert!(validate_output(&args, true).is_err());

        let args = ExtractArgs::try_parse_from([
            "extract",
            "--directory",
            "input",
            "--output",
            "output.pcap",
        ])
        .unwrap();
        assert_eq!(output_filename(&args), "output.pcap");
        assert!(validate_output(&args, true).is_ok());
    }

    #[test]
    fn test_parse_start_time() {
        assert_eq!(
            parse_start_time("20240101T00:00:00Z").unwrap(),
            1_704_067_200_000_000
        );
        assert_eq!(
            parse_start_time("20240101T00:00:00.000123Z").unwrap(),
            1_704_067_200_000_123
        );
        assert_eq!(
            parse_start_time("20240101T01:00:00+01:00").unwrap(),
            1_704_067_200_000_000
        );

        let local_midnight = chrono::Local
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp_micros() as u64;
        assert_eq!(parse_start_time("20240101").unwrap(), local_midnight);
        assert!(parse_start_time("1704067200").is_err());
    }
}
