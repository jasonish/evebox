// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! The blocking PCAP fetch entry point.

use crate::prelude::*;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use super::filter::{FlowSelector, vlan_wrapped};
use super::spool::{self, SpoolConfig};
use super::writer::PcapWriter;

/// The local packet capture input served by the extraction engine.
#[derive(Debug, Clone)]
pub(crate) enum PcapSource {
    /// A directory containing rotating packet capture files.
    Spool(SpoolConfig),
    /// Explicit packet capture files, without sibling discovery or
    /// filename-based time pruning.
    Files(Vec<PathBuf>),
}

/// Resource limits for a fetch.
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
    /// Link type of the first opened file, if any.
    pub(crate) linktype: Option<pcap::Linktype>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FetchError {
    /// No spool files could contain the requested time window, or every
    /// candidate vanished before it could be opened.
    #[error("no spool files match the requested time window")]
    NoCandidateFiles,
    /// Files were scanned, but no packets matched the request.
    #[error("no packets matched the request")]
    NoMatch(FetchStats),
    /// A malformed request, e.g. the BPF expression failed to compile.
    #[error("{0}")]
    Format(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Why the fetch stopped early.
#[derive(Clone, Copy)]
enum Stop {
    Cancelled,
    Deadline,
}

/// Cancellation and deadline tracking, checked with per-packet
/// granularity.
struct Control<'a> {
    cancel: &'a CancellationToken,
    started: Instant,
    deadline: Option<Duration>,
    stopped: Option<Stop>,
    /// Time spent in output writes and flushes, credited back so the
    /// deadline bounds scan time only: a slow but live consumer must
    /// not turn backpressure into a silent deadline truncation.
    write_credit: Duration,
}

impl Control<'_> {
    /// False once the fetch should stop.
    fn live(&mut self) -> bool {
        if self.stopped.is_some() {
            return false;
        }
        if self.cancel.is_cancelled() {
            self.stopped = Some(Stop::Cancelled);
            return false;
        }
        if let Some(deadline) = self.deadline
            && self.started.elapsed().saturating_sub(self.write_credit) >= deadline
        {
            self.stopped = Some(Stop::Deadline);
            return false;
        }
        true
    }
}

/// Periodic output flushing on the scan clock: matched packets held
/// below the output's buffering granularity are pushed out at least
/// every `interval`, so a consumer's time-to-first-byte tracks the
/// first MATCH, not the whole scan. Flush time is credited back to
/// the deadline like any other write.
struct Flusher {
    interval: Duration,
    last: Instant,
}

impl Flusher {
    fn maybe_flush(
        &mut self,
        writer: &mut PcapWriter,
        ctl: &mut Control,
    ) -> Result<(), FetchError> {
        if self.last.elapsed() < self.interval {
            return Ok(());
        }
        let flush_started = Instant::now();
        writer.flush()?;
        ctl.write_credit += flush_started.elapsed();
        self.last = Instant::now();
        Ok(())
    }
}

/// Per-fetch matching state shared by every cursor: the filter
/// renderings prepared per file and the packet time gate.
struct Matcher<'a> {
    /// A `PcapFilter::Expression`, compiled against each capture's
    /// actual link type.
    expression: Option<&'a str>,
    /// The (VLAN-wrapped, base) renderings of a `PcapFilter::Flow`,
    /// compiled per file.
    flow_expressions: &'a Option<(String, String)>,
    /// Only gate on packet timestamps when the request has a time
    /// window, preserving the historical extract behavior of passing
    /// through packets with unusual timestamps when no window was
    /// given.
    gate: bool,
    /// Inclusive window bounds, unix microseconds.
    start: u64,
    end: u64,
}

/// Compilation results for a user BPF expression across the candidate link
/// types. A link-type-specific expression may legitimately fail on some
/// files, but if it compiles on none of them the caller still receives a
/// format error rather than a misleading no-match result.
#[derive(Default)]
struct BpfExprTracker {
    compiled_any: bool,
    first_error: Option<String>,
}

impl BpfExprTracker {
    fn compiled(&mut self) {
        self.compiled_any = true;
    }

    fn failed(&mut self, err: &pcap::Error) {
        if self.first_error.is_none() {
            self.first_error = Some(err.to_string());
        }
    }
}

/// The heap key: the packet timestamp as unix microseconds, with
/// unrepresentable (negative or overflowing) timestamps saturated so
/// weird packets still merge deterministically when no time window
/// gates them out.
fn ts_micros(header: &pcap::PacketHeader) -> u64 {
    let secs = u64::try_from(header.ts.tv_sec).unwrap_or(0);
    let micros = u64::try_from(header.ts.tv_usec).unwrap_or(0);
    secs.saturating_mul(1_000_000).saturating_add(micros)
}

/// A matched packet held across capture reads. `pcap::Packet` borrows
/// the capture's internal buffer, so the merge's one pending packet
/// per group is an owned copy; only packets that pass the time gate
/// and the filter are ever buffered.
struct PendingPacket {
    ts_micros: u64,
    header: pcap::PacketHeader,
    data: Vec<u8>,
    /// Link type of the file the packet was read from.
    linktype: pcap::Linktype,
}

/// A capture open on one file of a group.
struct OpenCapture {
    capture: pcap::Capture<pcap::Offline>,
    linktype: pcap::Linktype,
    /// The request filter compiled against this file's link type and
    /// applied manually per packet.
    filter_program: Option<pcap::BpfProgram>,
    path: PathBuf,
}

/// The public `pcap::BpfProgram::filter` helper synthesizes a packet
/// header with `len == caplen`. Use libpcap's offline evaluator with
/// the real packet header so free-form filters using `len` retain
/// their normal libpcap semantics for snaplen-truncated packets.
#[repr(C)]
struct RawBpfProgram {
    bf_len: libc::c_uint,
    bf_insns: *const pcap::BpfInstruction,
}

unsafe extern "C" {
    #[link_name = "pcap_offline_filter"]
    fn offline_filter(
        program: *const RawBpfProgram,
        header: *const pcap::PacketHeader,
        data: *const libc::c_uchar,
    ) -> libc::c_int;
}

fn bpf_matches(program: &pcap::BpfProgram, packet: &pcap::Packet<'_>) -> bool {
    let instructions = program.get_instructions();
    let raw = RawBpfProgram {
        bf_len: instructions.len() as libc::c_uint,
        bf_insns: instructions.as_ptr(),
    };
    // SAFETY: `raw` points at the instructions owned by `program`, and
    // the packet header and data remain valid for the duration of the
    // call. Both structures have the C layouts expected by libpcap.
    unsafe { offline_filter(&raw, packet.header, packet.data.as_ptr()) > 0 }
}

/// Walks one rotation group's files in order, holding at most one
/// open capture and one pending matched packet.
struct GroupCursor {
    files: VecDeque<PathBuf>,
    open: Option<OpenCapture>,
    pending: Option<PendingPacket>,
}

impl GroupCursor {
    fn new(files: Vec<PathBuf>) -> Self {
        Self {
            files: files.into(),
            open: None,
            pending: None,
        }
    }

    /// The file the pending packet was read from: refill leaves the
    /// capture open on that file.
    fn current_path(&self) -> Option<&Path> {
        self.open.as_ref().map(|open| open.path.as_path())
    }

    /// Skip the rest of the current file; the next refill continues
    /// with the group's next file.
    fn skip_current_file(&mut self) {
        self.open = None;
    }

    /// Pull this group's next matching packet into `pending`, opening
    /// files as needed. Leaves `pending` empty when the group is
    /// exhausted or the fetch was stopped (see `ctl.stopped`). Scan
    /// time passes here, so the periodic flush runs per scanned
    /// packet: matched output already buffered downstream is
    /// delivered even while this group scans a long non-matching
    /// stretch.
    fn refill(
        &mut self,
        matcher: &Matcher,
        bpf_expr_tracker: &mut BpfExprTracker,
        ctl: &mut Control,
        stats: &mut FetchStats,
        writer: &mut PcapWriter,
        flusher: &mut Flusher,
    ) -> Result<(), FetchError> {
        loop {
            if !ctl.live() {
                return Ok(());
            }
            flusher.maybe_flush(writer, ctl)?;
            let Some(open) = self.open.as_mut() else {
                let Some(path) = self.files.pop_front() else {
                    return Ok(());
                };
                self.open = open_group_file(&path, matcher, bpf_expr_tracker, stats)?;
                continue;
            };
            match open.capture.next_packet() {
                Ok(pkt) => {
                    if matcher.gate {
                        let Ok(secs) = u64::try_from(pkt.header.ts.tv_sec) else {
                            continue;
                        };
                        let Ok(micros) = u64::try_from(pkt.header.ts.tv_usec) else {
                            continue;
                        };
                        let Some(timestamp) = secs
                            .checked_mul(1_000_000)
                            .and_then(|timestamp| timestamp.checked_add(micros))
                        else {
                            continue;
                        };
                        if timestamp < matcher.start {
                            continue;
                        }
                        if timestamp > matcher.end {
                            // PCAP records preserve capture order, but
                            // their timestamps need not be monotonic.
                            // Keep scanning for a later record whose
                            // timestamp is back inside the window.
                            continue;
                        }
                    }
                    if let Some(program) = &open.filter_program
                        && !bpf_matches(program, &pkt)
                    {
                        continue;
                    }
                    self.pending = Some(PendingPacket {
                        ts_micros: ts_micros(pkt.header),
                        header: *pkt.header,
                        data: pkt.data.to_vec(),
                        linktype: open.linktype,
                    });
                    return Ok(());
                }
                Err(err) => {
                    match err {
                        pcap::Error::NoMorePackets => {}
                        pcap::Error::PcapError(ref error)
                            // Truncation errors are expected.
                            if error.contains("truncated") => {}
                        _ => {
                            warn!("pcap-error: {}: {}", open.path.display(), err);
                        }
                    }
                    self.open = None;
                }
            }
        }
    }
}

/// Open one spool file with the request's filter compiled against the
/// file's link type. `Ok(None)` means the file was skipped (vanished,
/// or a link-type specific filter incompatibility) with the stats
/// updated; only file-descriptor exhaustion is an error.
fn open_group_file(
    path: &Path,
    matcher: &Matcher,
    bpf_expr_tracker: &mut BpfExprTracker,
    stats: &mut FetchStats,
) -> Result<Option<OpenCapture>, FetchError> {
    info!("Processing file {}", path.display());
    let capture = match pcap::Capture::from_file(path).map_err(anyhow::Error::from) {
        Ok(capture) => capture,
        Err(err) => {
            if fd_exhausted(&err) {
                // The merge holds one open capture per rotation
                // group, so descriptor exhaustion would fail every
                // remaining group's opens too, silently thinning the
                // output while looking like rotation races. Abort
                // loudly instead.
                return Err(FetchError::Io(std::io::Error::other(format!(
                    "{}: {}",
                    path.display(),
                    err
                ))));
            }
            // Don't let one unreadable file abort the whole fetch.
            warn!("Failed to open {}: {}", path.display(), err);
            stats.files_vanished += 1;
            return Ok(None);
        }
    };
    stats.files_scanned += 1;
    let linktype = capture.get_datalink();
    if stats.linktype.is_none() {
        stats.linktype = Some(linktype);
    }
    // Compile, but do not install, the filter. Applying it manually in
    // refill ensures next_packet() returns every packet, keeping the
    // timestamp gate, cancellation and deadline checks per-packet.
    let filter_program: Option<pcap::BpfProgram> = if let Some(expression) = matcher.expression {
        match capture.compile(expression, true) {
            Ok(program) => {
                bpf_expr_tracker.compiled();
                Some(program)
            }
            Err(err) => {
                // User expressions may be valid only for a particular
                // link type. Skip incompatible files, but remember the
                // error so an expression that compiles nowhere remains
                // a format error.
                bpf_expr_tracker.failed(&err);
                warn!(
                    "Skipping {}: failed to compile filter: {}",
                    path.display(),
                    err
                );
                return Ok(None);
            }
        }
    } else {
        // A Flow filter uses the VLAN-wrapped expression where
        // supported, otherwise the base expression (libpcap has no
        // VLAN support on raw IP, loopback, Linux cooked and similar
        // link types, where VLAN tags cannot occur anyway).
        match matcher.flow_expressions {
            Some((wrapped, base)) => match capture.compile(wrapped, true) {
                Ok(program) => Some(program),
                Err(_) => match capture.compile(base, true) {
                    Ok(program) => Some(program),
                    Err(err) => {
                        warn!(
                            "Skipping {}: failed to compile flow filter: {}",
                            path.display(),
                            err
                        );
                        return Ok(None);
                    }
                },
            },
            None => None,
        }
    };
    Ok(Some(OpenCapture {
        capture,
        linktype,
        filter_program,
        path: path.to_path_buf(),
    }))
}

/// True for capture open failures caused by file-descriptor
/// exhaustion (EMFILE/ENFILE). libpcap surfaces errno as strerror
/// text, so the text is matched as well as the raw errno.
fn fd_exhausted(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        #[cfg(unix)]
        if let Some(io) = cause.downcast_ref::<std::io::Error>()
            && matches!(io.raw_os_error(), Some(libc::EMFILE) | Some(libc::ENFILE))
        {
            return true;
        }
        // libpcap errors are strings shaped "<path>: <strerror>", so only
        // match the strerror phrase; bare "emfile"/"enfile" substrings
        // would false-positive on paths containing them.
        if cause
            .to_string()
            .to_ascii_lowercase()
            .contains("too many open files")
        {
            return true;
        }
    }
    false
}

/// Fetch packets matching `request` from the source, writing them as a
/// classic pcap stream to `out`.
///
/// In grouped mode (parseable rotation names) the rotation groups —
/// Suricata's per-capture-thread file sequences — are merged on packet
/// timestamp, so the output is globally time-ordered across groups.
/// In the directory-fallback mode (unparseable names) files are
/// processed sequentially in walk order and the output may not be
/// time-ordered.
///
/// Blocking; callers run it under spawn_blocking. Writes NOTHING to `out`
/// until the first matching packet, so zero bytes written means no match.
/// Once matched output exists, `out` is flushed at least every
/// `limits.flush_interval` of scan time so consumers see matches promptly.
/// `cancel` is checked between packets and between files; on cancellation
/// the fetch stops writing and returns Ok with the stats so far.
pub(crate) fn fetch(
    source: &PcapSource,
    request: &PcapRequest,
    out: &mut dyn Write,
    cancel: &CancellationToken,
) -> Result<FetchStats, FetchError> {
    let started = Instant::now();

    // The wrapped and base renderings of a Flow filter, compiled per
    // file below against the file's link type.
    let mut flow_expressions: Option<(String, String)> = None;

    // Generated flow filters must always compile for Ethernet; a failure
    // here is a generator bug and should fail before any file is touched.
    // User expressions are instead compiled against each candidate's
    // actual link type below, so valid link-type-specific syntax works.
    if let Some(PcapFilter::Flow(selector)) = &request.filter {
        let base = selector.to_bpf();
        let wrapped = vlan_wrapped(&base);
        let dead = pcap::Capture::dead(pcap::Linktype::ETHERNET)
            .map_err(|err| FetchError::Format(err.to_string()))?;
        dead.compile(&wrapped, true)
            .map_err(|err| FetchError::Format(err.to_string()))?;
        flow_expressions = Some((wrapped, base));
    }

    let start = request.start.unwrap_or(0);
    let end = request.end.unwrap_or(u64::MAX);

    // Grouped mode: one cursor per rotation group, merged on packet
    // timestamp below. Fallback mode: every file in walk order behind
    // a single cursor, which degenerates the merge to a sequential
    // scan — opening every fallback file at once could exhaust file
    // descriptors on unbounded file counts.
    let groups: Vec<Vec<PathBuf>> = match source {
        PcapSource::Spool(spool) => match spool::load_files(spool, start, end)? {
            Some(sorted) => {
                // Sort the groups so the output is deterministic.
                let mut groups: Vec<_> = sorted.into_iter().collect();
                groups.sort_by(|a, b| a.0.cmp(&b.0));
                groups
                    .into_iter()
                    .map(|(_id, files)| files.into_iter().map(|(_ts, path)| path).collect())
                    .collect()
            }
            None => {
                warn!(
                    "Failed to load sorted file list, processing will not be optimized and output may not be time ordered"
                );
                vec![spool::walk_files(
                    &spool.directory,
                    spool.prefix.as_deref(),
                )?]
            }
        },
        // Give each explicit capture its own cursor so packets from
        // different inputs are merged in timestamp order.
        PcapSource::Files(paths) => paths.iter().cloned().map(|path| vec![path]).collect(),
    };
    if groups.iter().all(|files| files.is_empty()) {
        return Err(FetchError::NoCandidateFiles);
    }

    let mut stats = FetchStats::default();
    let mut writer = PcapWriter::new(out);
    // The link type of the output, established by the first written
    // packet. Files with a different link type are skipped after that.
    let mut output_linktype: Option<pcap::Linktype> = None;
    let matcher = Matcher {
        expression: match &request.filter {
            Some(PcapFilter::Expression(expression)) => Some(expression.as_str()),
            _ => None,
        },
        flow_expressions: &flow_expressions,
        gate: request.start.is_some() || request.end.is_some(),
        start,
        end,
    };
    let mut bpf_expr_tracker = BpfExprTracker::default();
    let mut ctl = Control {
        cancel,
        started,
        deadline: request.limits.deadline,
        stopped: None,
        write_credit: Duration::ZERO,
    };
    let mut flusher = Flusher {
        interval: request.limits.flush_interval,
        last: started,
    };

    // K-way merge across the rotation groups: one buffered pending
    // packet per cursor and a min-heap of (timestamp, group index).
    // Ties pop in group order, so equal timestamps across groups are
    // deterministic.
    let mut cursors: Vec<GroupCursor> = groups.into_iter().map(GroupCursor::new).collect();
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
    for (index, cursor) in cursors.iter_mut().enumerate() {
        cursor.refill(
            &matcher,
            &mut bpf_expr_tracker,
            &mut ctl,
            &mut stats,
            &mut writer,
            &mut flusher,
        )?;
        if let Some(pending) = &cursor.pending {
            heap.push(Reverse((pending.ts_micros, index)));
        }
    }

    while let Some(Reverse((_, index))) = heap.pop() {
        if !ctl.live() {
            break;
        }
        flusher.maybe_flush(&mut writer, &mut ctl)?;
        let cursor = &mut cursors[index];
        let packet = cursor
            .pending
            .take()
            .expect("popped cursor has a pending packet");
        if let Some(output_linktype) = output_linktype
            && packet.linktype != output_linktype
        {
            // The pending packet always comes from the cursor's
            // current file; drop it and skip the rest of that file.
            if let Some(path) = cursor.current_path() {
                warn!(
                    "Skipping {} because its link type ({:?}) differs from the output file ({:?})",
                    path.display(),
                    packet.linktype,
                    output_linktype
                );
            }
            cursor.skip_current_file();
        } else {
            if stats
                .bytes
                .saturating_add(writer.next_write_size(packet.data.len()))
                > request.limits.max_bytes
            {
                stats.truncated = true;
                return Ok(stats);
            }
            // Writes are timed and credited back to the deadline: the
            // consumer may exert backpressure here (a slow download
            // client) and that must not count as scan time.
            let write_started = Instant::now();
            writer.write_packet(
                packet.linktype,
                &pcap::Packet::new(&packet.header, &packet.data),
            )?;
            ctl.write_credit += write_started.elapsed();
            stats.bytes = writer.bytes_written();
            stats.packets += 1;
            output_linktype.get_or_insert(packet.linktype);
        }
        cursor.refill(
            &matcher,
            &mut bpf_expr_tracker,
            &mut ctl,
            &mut stats,
            &mut writer,
            &mut flusher,
        )?;
        if let Some(pending) = &cursor.pending {
            heap.push(Reverse((pending.ts_micros, index)));
        }
    }

    match ctl.stopped {
        Some(Stop::Cancelled) => return Ok(stats),
        Some(Stop::Deadline) => {
            stats.truncated = true;
            return Ok(stats);
        }
        None => {}
    }

    if stats.packets == 0 && !stats.truncated {
        if matcher.expression.is_some()
            && !bpf_expr_tracker.compiled_any
            && let Some(err) = bpf_expr_tracker.first_error
        {
            return Err(FetchError::Format(err));
        }
        if stats.files_scanned == 0 {
            // Every candidate vanished before it could be opened.
            return Err(FetchError::NoCandidateFiles);
        }
        return Err(FetchError::NoMatch(stats));
    }
    Ok(stats)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pcap::testutil::{
        LINKTYPE_RAW, count_packets, ipv4_datagram, ipv4_packet, ports, write_pcap_file,
        write_pcap_file_bytes, write_pcap_file_with_linktype, write_raw_pcap_file,
    };
    use std::path::Path;

    // Sizes of the pcap stream elements for the test UDP packet, used by
    // the max_bytes tests: 24 byte file header, 16 byte record header and
    // 42 bytes of packet data.
    const HEADER_SIZE: u64 = 24;
    const RECORD_SIZE: u64 = 16 + 42;

    fn fetch_to_vec(
        spool: &SpoolConfig,
        request: &PcapRequest,
    ) -> (Result<FetchStats, FetchError>, Vec<u8>) {
        let mut out = vec![];
        let cancel = CancellationToken::new();
        let source = PcapSource::Spool(spool.clone());
        let result = fetch(&source, request, &mut out, &cancel);
        (result, out)
    }

    /// Write the fetched bytes to a file and count the packets with
    /// libpcap, validating the output format at the same time.
    fn count_packets_in(dir: &Path, bytes: &[u8]) -> usize {
        let path = dir.join("fetch-output.pcap");
        std::fs::write(&path, bytes).unwrap();
        count_packets(&path)
    }

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tempdir = tempfile::tempdir().unwrap();
        let input = tempdir.path().join("input");
        std::fs::create_dir(&input).unwrap();
        (tempdir, input)
    }

    #[test]
    fn test_fetch_with_filter() {
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[
                (1_700_000_000, 53),
                (1_700_000_001, 9999),
                (1_700_000_002, 53),
            ],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Expression("udp and port 53".to_string())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.packets, 2);
        assert_eq!(stats.files_scanned, 1);
        assert_eq!(stats.files_vanished, 0);
        assert!(!stats.truncated);
        assert_eq!(stats.bytes, out.len() as u64);
        assert_eq!(stats.linktype, Some(pcap::Linktype::ETHERNET));
        assert_eq!(count_packets_in(tempdir.path(), &out), 2);
    }

    #[test]
    fn explicit_files_are_merged_without_scanning_siblings() {
        let (tempdir, input) = setup();
        // These selected filenames are deliberately later than the requested
        // window. Explicit files must not use spool filename pruning.
        let first = input.join("log.pcap.1.1900000000");
        let second = input.join("log.pcap.2.1900000000");
        write_pcap_file(&first, &[(1_700_000_010, 1010), (1_700_000_030, 1030)]);
        write_pcap_file(&second, &[(1_700_000_020, 1020), (1_700_000_040, 1040)]);
        // A matching sibling must not be discovered.
        write_pcap_file(
            &input.join("log.pcap.3.1700000000"),
            &[(1_700_000_025, 1025)],
        );

        let source = PcapSource::Files(vec![first, second]);
        let request = PcapRequest {
            start: Some(micros(1_700_000_010)),
            end: Some(micros(1_700_000_040)),
            ..Default::default()
        };
        let mut out = vec![];
        let stats = fetch(&source, &request, &mut out, &CancellationToken::new()).unwrap();

        assert_eq!(stats.files_scanned, 2);
        assert_eq!(stats.packets, 4);
        assert_eq!(
            read_timestamps(tempdir.path(), &out),
            vec![
                micros(1_700_000_010),
                micros(1_700_000_020),
                micros(1_700_000_030),
                micros(1_700_000_040),
            ]
        );
        assert_eq!(
            read_dest_ports(tempdir.path(), &out),
            vec![1010, 1020, 1030, 1040]
        );
    }

    #[test]
    fn test_expression_filter_compiles_for_actual_linktype() {
        let (tempdir, input) = setup();
        write_pcap_file(&input.join("a.pcap.1700000000"), &[(1_700_000_000, 53)]);
        write_pcap_file_with_linktype(
            &input.join("b.pcap.1700000000"),
            &[(1_700_000_000, 53)],
            pcap::Linktype::IEEE802_11,
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            // Valid for IEEE 802.11, but not Ethernet.
            filter: Some(PcapFilter::Expression("wlan type mgt".to_string())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.packets, 1);
        assert_eq!(stats.files_scanned, 2);

        let output = tempdir.path().join("linktype-output.pcap");
        std::fs::write(&output, out).unwrap();
        let capture = pcap::Capture::from_file(&output).unwrap();
        assert_eq!(capture.get_datalink(), pcap::Linktype::IEEE802_11);
        assert_eq!(count_packets(&output), 1);
    }

    #[test]
    fn test_expression_filter_malformed_for_every_linktype() {
        let (_tempdir, input) = setup();
        write_pcap_file(&input.join("a.pcap.1700000000"), &[(1_700_000_000, 53)]);
        write_pcap_file_with_linktype(
            &input.join("b.pcap.1700000000"),
            &[(1_700_000_000, 53)],
            pcap::Linktype::IEEE802_11,
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Expression("udp and (".to_string())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        assert!(matches!(result, Err(FetchError::Format(_))));
        assert!(out.is_empty());
    }

    #[test]
    fn test_expression_filter_is_applied_per_packet() {
        // The capture itself must remain unfiltered: otherwise
        // next_packet() can scan an arbitrary number of non-matches
        // inside libpcap without returning to the control checks in
        // refill.
        let (_tempdir, input) = setup();
        let path = input.join("log.pcap.1.1700000000");
        write_pcap_file(&path, &[(1_700_000_000, 9999), (1_700_000_001, 53)]);

        let flow_expressions = None;
        let matcher = Matcher {
            expression: Some("udp and port 53"),
            flow_expressions: &flow_expressions,
            gate: false,
            start: 0,
            end: u64::MAX,
        };
        let mut stats = FetchStats::default();
        let mut bpf_expr_tracker = BpfExprTracker::default();
        let mut open = open_group_file(&path, &matcher, &mut bpf_expr_tracker, &mut stats)
            .unwrap()
            .unwrap();

        let first = open.capture.next_packet().unwrap();
        assert_eq!(u16::from_be_bytes([first.data[36], first.data[37]]), 9999);
        assert!(!bpf_matches(open.filter_program.as_ref().unwrap(), &first));
    }

    #[test]
    fn test_expression_filter_uses_original_packet_length() {
        // A snaplen-truncated record has 42 captured bytes but an
        // original wire length of 100. Manual filtering must pass the
        // real packet header to libpcap so `len` keeps its documented
        // meaning.
        let (tempdir, input) = setup();
        let path = input.join("log.pcap.1.1700000000");
        write_pcap_file(&path, &[(1_700_000_000, 53)]);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[36..40].copy_from_slice(&100_u32.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Expression("len = 100".to_string())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.packets, 1);
        assert_eq!(count_packets_in(tempdir.path(), &out), 1);
    }

    #[test]
    fn test_fetch_time_window() {
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[
                (1_700_000_000, 53),
                (1_700_000_100, 53),
                (1_700_000_200, 53),
            ],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            start: Some(1_700_000_050_000_000),
            end: Some(1_700_000_150_000_000),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.packets, 1);
        assert_eq!(count_packets_in(tempdir.path(), &out), 1);
    }

    #[test]
    fn test_fetch_time_window_scans_out_of_order_records() {
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[(1_700_000_200, 9999), (1_700_000_100, 53)],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            start: Some(1_700_000_050_000_000),
            end: Some(1_700_000_150_000_000),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.packets, 1);
        assert_eq!(count_packets_in(tempdir.path(), &out), 1);
    }

    #[test]
    fn test_fetch_max_bytes_truncation() {
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[(1_700_000_000, 53), (1_700_000_001, 53)],
        );

        let spool = SpoolConfig::new(&input, None);
        // Enough for the file header and one record but not two.
        let request = PcapRequest {
            limits: Limits {
                max_bytes: HEADER_SIZE + RECORD_SIZE + RECORD_SIZE - 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert!(stats.truncated);
        assert_eq!(stats.packets, 1);
        assert_eq!(stats.bytes, HEADER_SIZE + RECORD_SIZE);
        assert_eq!(count_packets_in(tempdir.path(), &out), 1);
    }

    #[test]
    fn test_fetch_first_match_over_cap() {
        let (_tempdir, input) = setup();
        write_pcap_file(&input.join("log.pcap.1.1700000000"), &[(1_700_000_000, 53)]);

        let spool = SpoolConfig::new(&input, None);
        // The very first match does not fit: empty output, truncated, Ok.
        let request = PcapRequest {
            limits: Limits {
                max_bytes: HEADER_SIZE + RECORD_SIZE - 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert!(stats.truncated);
        assert_eq!(stats.packets, 0);
        assert_eq!(stats.bytes, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_fetch_deadline() {
        let (_tempdir, input) = setup();
        write_pcap_file(&input.join("log.pcap.1.1700000000"), &[(1_700_000_000, 53)]);

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            limits: Limits {
                max_bytes: u64::MAX,
                deadline: Some(Duration::ZERO),
                ..Default::default()
            },
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert!(stats.truncated);
        assert_eq!(stats.packets, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_fetch_cancellation() {
        let (_tempdir, input) = setup();
        write_pcap_file(&input.join("log.pcap.1.1700000000"), &[(1_700_000_000, 53)]);

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest::default();
        let mut out = vec![];
        let cancel = CancellationToken::new();
        cancel.cancel();
        let stats = fetch(&PcapSource::Spool(spool), &request, &mut out, &cancel).unwrap();
        assert_eq!(stats.packets, 0);
        assert!(!stats.truncated);
        assert!(out.is_empty());
    }

    #[test]
    fn test_fetch_no_match_carries_stats() {
        let (_tempdir, input) = setup();
        write_pcap_file(&input.join("log.pcap.1.1700000000"), &[(1_700_000_000, 53)]);

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Expression("udp and port 9999".to_string())),
            ..Default::default()
        };
        let (result, _out) = fetch_to_vec(&spool, &request);
        match result {
            Err(FetchError::NoMatch(stats)) => {
                assert_eq!(stats.packets, 0);
                assert_eq!(stats.bytes, 0);
                assert_eq!(stats.files_scanned, 1);
                assert!(!stats.truncated);
                assert_eq!(stats.linktype, Some(pcap::Linktype::ETHERNET));
            }
            other => panic!("expected NoMatch, got {:?}", other),
        }
    }

    #[test]
    fn test_fetch_no_candidate_files() {
        // An empty spool directory.
        let (_tempdir, input) = setup();
        let spool = SpoolConfig::new(&input, None);
        let (result, out) = fetch_to_vec(&spool, &PcapRequest::default());
        assert!(matches!(result, Err(FetchError::NoCandidateFiles)));
        assert!(out.is_empty());

        // All files pruned by the time window (zero margin).
        write_pcap_file(&input.join("log.pcap.1.1700000000"), &[(1_700_000_000, 53)]);
        let spool = SpoolConfig {
            margin: Duration::ZERO,
            ..SpoolConfig::new(&input, None)
        };
        let request = PcapRequest {
            start: Some(1_600_000_000_000_000),
            end: Some(1_600_000_100_000_000),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        assert!(matches!(result, Err(FetchError::NoCandidateFiles)));
        assert!(out.is_empty());
    }

    fn udp_flow_selector() -> FlowSelector {
        FlowSelector {
            proto: 17,
            a: ("10.1.1.5".parse().unwrap(), Some(4000)),
            b: ("192.0.2.10".parse().unwrap(), Some(53)),
        }
    }

    #[test]
    fn test_fetch_flow_filter_raw_linktype() {
        // A raw-IP (LINKTYPE_RAW) spool, as Suricata pcap-log writes
        // for tun and similar interfaces: libpcap cannot compile the
        // VLAN-wrapped expression for this link type, so the fetch
        // must fall back to the base expression and still find the
        // flow's packets.
        let (tempdir, input) = setup();
        write_pcap_file_bytes(
            &input.join("log.pcap.1.1700000000"),
            LINKTYPE_RAW,
            &[
                ipv4_datagram(17, "10.1.1.5", "192.0.2.10", &ports(4000, 53)),
                ipv4_datagram(17, "192.0.2.10", "10.1.1.5", &ports(53, 4000)),
                ipv4_datagram(17, "10.1.1.6", "192.0.2.10", &ports(4000, 53)),
            ],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Flow(udp_flow_selector())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.packets, 2);
        assert_eq!(stats.files_scanned, 1);
        assert_ne!(stats.linktype, Some(pcap::Linktype::ETHERNET));

        // The output keeps the raw-IP link type and is readable.
        let path = tempdir.path().join("fetch-output.pcap");
        std::fs::write(&path, &out).unwrap();
        let capture = pcap::Capture::from_file(&path).unwrap();
        assert_eq!(Some(capture.get_datalink()), stats.linktype);
        assert_eq!(count_packets(&path), 2);
    }

    #[test]
    fn test_fetch_flow_filter_mixed_linktypes() {
        // A raw-IP file and an Ethernet file in one fetch: the flow
        // filter is compiled per file against each file's link type.
        // The raw file comes first and establishes the output link
        // type, so the Ethernet file is skipped by the link type
        // gate, not by a filter failure.
        let (tempdir, input) = setup();
        write_pcap_file_bytes(
            &input.join("a.pcap.1700000000"),
            LINKTYPE_RAW,
            &[ipv4_datagram(
                17,
                "10.1.1.5",
                "192.0.2.10",
                &ports(4000, 53),
            )],
        );
        write_raw_pcap_file(
            &input.join("b.pcap.1700000000"),
            &[ipv4_packet(17, "10.1.1.5", "192.0.2.10", &ports(4000, 53))],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Flow(udp_flow_selector())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.files_scanned, 2);
        assert_eq!(stats.files_vanished, 0);
        assert_eq!(stats.packets, 1);
        assert_eq!(count_packets_in(tempdir.path(), &out), 1);
    }

    #[test]
    fn test_fetch_all_candidates_vanished() {
        // Every candidate failed to open (rotation race or
        // corruption): no-candidate-files, not no-match.
        let (_tempdir, input) = setup();
        std::fs::write(input.join("log.pcap.1.1700000000"), "not a pcap file").unwrap();
        let spool = SpoolConfig::new(&input, None);
        let (result, out) = fetch_to_vec(&spool, &PcapRequest::default());
        assert!(matches!(result, Err(FetchError::NoCandidateFiles)));
        assert!(out.is_empty());
    }

    #[test]
    fn test_fetch_counts_vanished_files() {
        let (tempdir, input) = setup();
        write_pcap_file(&input.join("log.pcap.1.1700000000"), &[(1_700_000_000, 53)]);
        // A file that groups like a spool file but is not a valid capture.
        std::fs::write(input.join("log.pcap.2.1700000000"), "not a pcap file").unwrap();

        let spool = SpoolConfig::new(&input, None);
        let (result, out) = fetch_to_vec(&spool, &PcapRequest::default());
        let stats = result.unwrap();
        assert_eq!(stats.packets, 1);
        assert_eq!(stats.files_scanned, 1);
        assert_eq!(stats.files_vanished, 1);
        assert_eq!(count_packets_in(tempdir.path(), &out), 1);
    }

    #[test]
    fn test_fetch_skips_mismatched_linktype() {
        let (tempdir, input) = setup();
        write_pcap_file(&input.join("a.pcap.1700000000"), &[(1_700_000_000, 53)]);
        write_pcap_file_with_linktype(
            &input.join("b.pcap.1700000000"),
            &[(1_700_000_000, 53)],
            pcap::Linktype::NULL,
        );

        let spool = SpoolConfig::new(&input, None);
        let (result, out) = fetch_to_vec(&spool, &PcapRequest::default());
        let stats = result.unwrap();
        assert_eq!(stats.packets, 1);
        assert_eq!(stats.files_scanned, 2);
        assert_eq!(stats.files_vanished, 0);

        let path = tempdir.path().join("fetch-output.pcap");
        std::fs::write(&path, &out).unwrap();
        let capture = pcap::Capture::from_file(&path).unwrap();
        assert_eq!(capture.get_datalink(), pcap::Linktype::ETHERNET);
        assert_eq!(count_packets(&path), 1);
    }

    /// Seconds offset from the spool epoch as unix microseconds.
    fn micros(seconds: i64) -> u64 {
        u64::try_from(seconds).unwrap() * 1_000_000
    }

    /// Read the fetched bytes back with libpcap, returning each
    /// packet's timestamp in unix microseconds, in stream order.
    fn read_timestamps(dir: &Path, bytes: &[u8]) -> Vec<u64> {
        let path = dir.join("timestamps.pcap");
        std::fs::write(&path, bytes).unwrap();
        let mut capture = pcap::Capture::from_file(&path).unwrap();
        let mut timestamps = vec![];
        while let Ok(pkt) = capture.next_packet() {
            timestamps.push(ts_micros(pkt.header));
        }
        timestamps
    }

    /// Read the fetched bytes back with libpcap, returning each UDP
    /// packet's destination port, in stream order.
    fn read_dest_ports(dir: &Path, bytes: &[u8]) -> Vec<u16> {
        let path = dir.join("ports.pcap");
        std::fs::write(&path, bytes).unwrap();
        let mut capture = pcap::Capture::from_file(&path).unwrap();
        let mut ports = vec![];
        while let Ok(pkt) = capture.next_packet() {
            // Ethernet/IPv4/UDP: destination port at offset 36.
            ports.push(u16::from_be_bytes([pkt.data[36], pkt.data[37]]));
        }
        ports
    }

    #[test]
    fn test_fetch_merges_groups_in_time_order() {
        // Two capture-thread rotation groups with interleaved
        // timestamps: the output is the global time order, not one
        // group's timeline after the other.
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[
                (1_700_000_010, 53),
                (1_700_000_011, 9999),
                (1_700_000_030, 53),
                (1_700_000_050, 53),
            ],
        );
        write_pcap_file(
            &input.join("log.pcap.2.1700000000"),
            &[
                (1_700_000_015, 9999),
                (1_700_000_020, 53),
                (1_700_000_040, 53),
            ],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Expression("udp and port 53".to_string())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert_eq!(stats.packets, 5);
        assert_eq!(stats.files_scanned, 2);
        assert_eq!(
            read_timestamps(tempdir.path(), &out),
            vec![
                micros(1_700_000_010),
                micros(1_700_000_020),
                micros(1_700_000_030),
                micros(1_700_000_040),
                micros(1_700_000_050),
            ]
        );
    }

    #[test]
    fn test_fetch_merge_equal_timestamps_stable() {
        // Equal timestamps across groups pop in group order: the
        // output is deterministic, pinned here by destination port.
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[(1_700_000_010, 1111), (1_700_000_020, 1111)],
        );
        write_pcap_file(
            &input.join("log.pcap.2.1700000000"),
            &[(1_700_000_010, 2222), (1_700_000_020, 2222)],
        );

        let spool = SpoolConfig::new(&input, None);
        let (result, out) = fetch_to_vec(&spool, &PcapRequest::default());
        let stats = result.unwrap();
        assert_eq!(stats.packets, 4);
        assert_eq!(
            read_timestamps(tempdir.path(), &out),
            vec![
                micros(1_700_000_010),
                micros(1_700_000_010),
                micros(1_700_000_020),
                micros(1_700_000_020),
            ]
        );
        assert_eq!(
            read_dest_ports(tempdir.path(), &out),
            vec![1111, 2222, 1111, 2222]
        );
    }

    #[test]
    fn test_fetch_merge_skips_mismatched_linktype_file() {
        // A link-type mismatched file inside the merge is dropped at
        // its first packet and the remaining groups keep merging.
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[(1_700_000_010, 53), (1_700_000_030, 53)],
        );
        write_pcap_file_with_linktype(
            &input.join("log.pcap.2.1700000000"),
            &[(1_700_000_020, 53), (1_700_000_040, 53)],
            pcap::Linktype::NULL,
        );
        write_pcap_file(
            &input.join("log.pcap.3.1700000000"),
            &[(1_700_000_015, 53), (1_700_000_035, 53)],
        );

        let spool = SpoolConfig::new(&input, None);
        let (result, out) = fetch_to_vec(&spool, &PcapRequest::default());
        let stats = result.unwrap();
        assert_eq!(stats.packets, 4);
        assert_eq!(stats.files_scanned, 3);
        assert_eq!(stats.files_vanished, 0);
        assert_eq!(
            read_timestamps(tempdir.path(), &out),
            vec![
                micros(1_700_000_010),
                micros(1_700_000_015),
                micros(1_700_000_030),
                micros(1_700_000_035),
            ]
        );
    }

    #[test]
    fn test_fetch_merge_max_bytes_truncation() {
        // The byte cap stops the merge at the right packet: the two
        // earliest packets across groups are written, the third —
        // back in the first group — does not fit.
        let (tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[(1_700_000_010, 53), (1_700_000_030, 53)],
        );
        write_pcap_file(
            &input.join("log.pcap.2.1700000000"),
            &[(1_700_000_020, 53), (1_700_000_040, 53)],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            limits: Limits {
                max_bytes: HEADER_SIZE + 2 * RECORD_SIZE,
                ..Default::default()
            },
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        let stats = result.unwrap();
        assert!(stats.truncated);
        assert_eq!(stats.packets, 2);
        assert_eq!(stats.bytes, HEADER_SIZE + 2 * RECORD_SIZE);
        assert_eq!(
            read_timestamps(tempdir.path(), &out),
            vec![micros(1_700_000_010), micros(1_700_000_020)]
        );
    }

    #[test]
    fn test_fd_exhaustion_classification() {
        // Rotation races and per-file faults keep the
        // skip-with-warning contract; only descriptor exhaustion
        // aborts the fetch.
        assert!(!fd_exhausted(&anyhow!("No such file or directory")));
        assert!(!fd_exhausted(&anyhow::Error::from(std::io::Error::from(
            std::io::ErrorKind::NotFound
        ))));
        // libpcap surfaces errno as strerror text.
        assert!(fd_exhausted(&anyhow!("log.pcap: Too Many Open Files")));
        #[cfg(unix)]
        {
            assert!(fd_exhausted(&anyhow::Error::from(
                std::io::Error::from_raw_os_error(libc::EMFILE)
            )));
            assert!(fd_exhausted(&anyhow::Error::from(
                std::io::Error::from_raw_os_error(libc::ENFILE)
            )));
        }
    }

    #[test]
    fn test_fetch_writes_nothing_before_first_match() {
        let (_tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[(1_700_000_000, 53), (1_700_000_001, 53)],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Expression("tcp".to_string())),
            ..Default::default()
        };
        let (result, out) = fetch_to_vec(&spool, &request);
        assert!(matches!(result, Err(FetchError::NoMatch(_))));
        assert!(out.is_empty());
    }

    /// A writer that blocks in `write` the way a backpressured
    /// download does: the time spent there must be credited back to
    /// the deadline.
    struct SlowWriter {
        out: Vec<u8>,
        delay: Duration,
    }

    impl Write for SlowWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            std::thread::sleep(self.delay);
            self.out.extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_deadline_excludes_output_write_time() {
        // Six packets at 80ms of write time each: elapsed wall time
        // blows well past the 200ms deadline, but almost none of it
        // is scan time, so the fetch must complete untruncated.
        let (_tempdir, input) = setup();
        let packets: Vec<(i64, u16)> = (0..6).map(|i| (1_700_000_000 + i, 53)).collect();
        write_pcap_file(&input.join("log.pcap.1.1700000000"), &packets);

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            limits: Limits {
                deadline: Some(Duration::from_millis(200)),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut out = SlowWriter {
            out: vec![],
            delay: Duration::from_millis(80),
        };
        let cancel = CancellationToken::new();
        let stats = fetch(&PcapSource::Spool(spool), &request, &mut out, &cancel).unwrap();
        assert!(!stats.truncated, "write time counted against the deadline");
        assert_eq!(stats.packets, 6);
        assert_eq!(stats.bytes, out.out.len() as u64);
    }

    /// Records the output length seen at each flush.
    struct CountingWriter {
        out: Vec<u8>,
        flush_lens: Vec<usize>,
    }

    impl Write for CountingWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.out.extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.flush_lens.push(self.out.len());
            Ok(())
        }
    }

    #[test]
    fn test_periodic_flush_delivers_between_packets() {
        // A zero flush interval flushes on every scan checkpoint:
        // the output must be flushed at a point where only the first
        // of the two records has been written, proving delivery does
        // not wait for fetch completion.
        let (_tempdir, input) = setup();
        write_pcap_file(
            &input.join("log.pcap.1.1700000000"),
            &[(1_700_000_000, 53), (1_700_000_001, 53)],
        );

        let spool = SpoolConfig::new(&input, None);
        let request = PcapRequest {
            limits: Limits {
                flush_interval: Duration::ZERO,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut out = CountingWriter {
            out: vec![],
            flush_lens: vec![],
        };
        let cancel = CancellationToken::new();
        let stats = fetch(&PcapSource::Spool(spool), &request, &mut out, &cancel).unwrap();
        assert_eq!(stats.packets, 2);
        let one_record = (HEADER_SIZE + RECORD_SIZE) as usize;
        assert!(
            out.flush_lens.contains(&one_record),
            "no flush between the two packets: {:?}",
            out.flush_lens
        );
    }
}
