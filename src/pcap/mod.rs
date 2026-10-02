// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! PCAP extraction engine.
//!
//! Extracts packets matching a filter and time window from explicit
//! captures or a directory of PCAP spool files (for example,
//! Suricata's pcap-log output) into a classic pcap stream. The
//! blocking [`fetch`] entry point is shared by the `evebox pcap
//! extract` command and the server API.

// Unix links libpcap as before; Windows loads Npcap only when needed.
mod backend;
mod fetch;
mod filter;
mod request;
mod spool;
mod timeframe;
mod writer;

#[cfg(all(test, not(windows)))]
pub(crate) mod testutil;

pub(crate) use backend::ensure_available;
pub(crate) use fetch::{FetchError, fetch};
pub(crate) use filter::FlowSelector;
pub(crate) use request::{FetchStats, Limits, PcapFilter, PcapRequest, PcapSource, SpoolConfig};
pub(crate) use spool::walk_files;
pub(crate) use timeframe::{
    Window, derive_window, selector_from_event, window_around_event, window_from_start,
};
