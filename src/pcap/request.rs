// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Portable request types for the extraction engine.
//!
//! These carry no libpcap dependency: a Windows server uses them to
//! describe a fetch dispatched to a remote agent, while the
//! libpcap-backed [`super::fetch`] entry point is compiled out.

use std::path::PathBuf;
use std::time::Duration;

use super::filter::FlowSelector;

/// A directory of PCAP spool files to extract packets from.
#[cfg_attr(windows, allow(dead_code))]
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

/// The local packet capture input served by the extraction engine.
#[cfg_attr(windows, allow(dead_code))]
#[derive(Debug, Clone)]
pub(crate) enum PcapSource {
    /// A directory containing rotating packet capture files.
    Spool(SpoolConfig),
    /// Explicit packet capture files, without sibling discovery or
    /// filename-based time pruning.
    Files(Vec<PathBuf>),
}

/// Resource limits for a fetch.
#[cfg_attr(windows, allow(dead_code))]
#[derive(Debug, Clone)]
pub(crate) struct Limits {
    /// Maximum number of bytes to write to the output, including the pcap
    /// file header.
    pub(crate) max_bytes: u64,
    /// Cap on SCAN time. Time spent writing or flushing the output —
    /// including consumer backpressure, e.g. a slow but live download
    /// client — does not count against it.
    pub(crate) deadline: Option<Duration>,
    /// Output is flushed at least this often (in scan time), so
    /// consumers see early matches promptly even when the output
    /// buffers below its transport's chunk size.
    pub(crate) flush_interval: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: u64::MAX,
            deadline: None,
            flush_interval: Duration::from_millis(250),
        }
    }
}

/// The packet filter of a [`PcapRequest`].
#[derive(Debug)]
pub(crate) enum PcapFilter {
    /// A user-supplied BPF expression (libpcap syntax), compiled for
    /// each capture's link type and applied manually per packet so
    /// cancellation, the deadline and the time gate remain
    /// interruptible. Files whose link type rejects the expression
    /// are skipped.
    Expression(String),
    /// A flow selector, rendered to BPF per file: the VLAN-wrapped
    /// expression where the file's link type supports the `vlan`
    /// keyword, falling back to the unwrapped expression on link
    /// types without VLAN support (raw IP, loopback, Linux cooked,
    /// ...). The compiled program is applied manually per packet so
    /// cancellation and the deadline are observed with per-packet
    /// granularity.
    Flow(FlowSelector),
}

/// Statistics from a completed or partially-completed fetch.
#[cfg_attr(windows, allow(dead_code))]
#[derive(Debug, Default)]
pub(crate) struct FetchStats {
    /// Number of packets written to the output.
    pub(crate) packets: u64,
    /// Bytes written to the output, including the pcap file header.
    pub(crate) bytes: u64,
    /// Files successfully opened.
    pub(crate) files_scanned: u32,
    /// Files that failed to open or read (rotation race).
    pub(crate) files_vanished: u32,
    /// The fetch was stopped by max_bytes or the deadline.
    pub(crate) truncated: bool,
    /// Raw link type of the first opened file, if any. Kept as the
    /// bare integer so this type carries no libpcap dependency.
    pub(crate) linktype: Option<i32>,
}

/// A request for packets from a PCAP spool.
#[derive(Debug, Default)]
pub(crate) struct PcapRequest {
    /// The packet filter to apply.
    pub(crate) filter: Option<PcapFilter>,
    /// Inclusive start of the time window, unix microseconds.
    pub(crate) start: Option<u64>,
    /// Inclusive end of the time window, unix microseconds.
    pub(crate) end: Option<u64>,
    pub(crate) limits: Limits,
}
