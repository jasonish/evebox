// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

#[cfg(not(windows))]
pub(crate) mod channel;
pub(crate) mod client;
pub(crate) mod importer;
pub(crate) mod protocol;
pub(crate) mod tls;
