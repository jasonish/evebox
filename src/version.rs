// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

pub const VERSION: &str = std::env!("CARGO_PKG_VERSION");
pub const TARGET: Option<&str> = std::option_env!("TARGET");
pub const BUILD_REV: Option<&str> = std::option_env!("BUILD_REV");

pub fn version() -> &'static str {
    VERSION
}

pub fn target() -> &'static str {
    TARGET.unwrap_or("unknown")
}

pub fn build_rev() -> &'static str {
    BUILD_REV.unwrap_or("unknown")
}

pub fn log_version() {
    info!(
        "This is EveBox version {} (rev: {}); {}",
        version(),
        build_rev(),
        target(),
    );
}

pub fn print_version() {
    println!(
        "EveBox Version {} (rev {}); {}",
        VERSION,
        BUILD_REV.unwrap_or("unknown"),
        TARGET.unwrap_or("unknown"),
    );
}
