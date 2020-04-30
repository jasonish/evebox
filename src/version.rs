// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use crate::logger::log;

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
    log::info!(
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
