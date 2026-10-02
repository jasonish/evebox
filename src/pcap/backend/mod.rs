// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Offline capture API: linked libpcap on Unix, optional Npcap on Windows.
//! Keep the Unix crate and its existing static release linkage unchanged.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use unix::*;
#[cfg(windows)]
pub(crate) use windows::*;
