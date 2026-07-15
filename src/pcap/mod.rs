// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! PCAP extraction engine.
//!
//! Extracts packets matching a filter and time window from explicit
//! captures or a directory of PCAP spool files (for example,
//! Suricata's pcap-log output) into a classic pcap stream. The
//! blocking [`fetch`] entry point is shared by the `evebox pcap
//! extract` command and the server API.

// The blocking extraction path links libpcap and is compiled out on
// Windows; the request/filter/spool/timeframe types stay portable so a
// Windows server can describe fetches dispatched to remote agents.
#[cfg(not(windows))]
mod fetch;
mod filter;
mod request;
#[cfg(not(windows))]
mod spool;
mod timeframe;
#[cfg(not(windows))]
mod writer;

#[cfg(all(test, not(windows)))]
pub(crate) mod testutil;

#[cfg(not(windows))]
pub(crate) use fetch::{FetchError, fetch};
pub(crate) use filter::FlowSelector;
pub(crate) use request::{FetchStats, Limits, PcapFilter, PcapRequest, PcapSource, SpoolConfig};
#[cfg(not(windows))]
pub(crate) use spool::walk_files;
pub(crate) use timeframe::{
    Window, derive_window, selector_from_event, window_around_event, window_from_start,
};
