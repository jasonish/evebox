// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

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
