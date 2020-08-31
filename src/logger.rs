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

// Export the standard log style marcros under the log module. To use:
//    use crate::logger::log;
//    log::info("Something info...");
pub mod log {
    pub use tracing::debug;
    pub use tracing::error;
    pub use tracing::info;
    pub use tracing::trace;
    pub use tracing::warn;
}

use tracing::Level;

pub fn init_logger(level: Level) {
    let level = match level {
        Level::TRACE => "trace",
        Level::DEBUG => "debug",
        Level::INFO => "info",
        Level::WARN => "warn",
        Level::ERROR => "error",
    };
    let timer =
        tracing_subscriber::fmt::time::ChronoLocal::with_format("%Y-%m-%d %H:%M:%S".to_string());
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(format!("{},hyper=off,warp=off", level))
        .with_writer(std::io::stderr)
        .with_timer(timer)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

pub fn init_stdlog() {
    tracing_log::LogTracer::builder()
        .with_max_level(stdlog::LevelFilter::Info)
        .init()
        .unwrap();
}
