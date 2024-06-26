// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use time::macros::format_description;
use time::UtcOffset;
pub use tracing::debug;
pub use tracing::error;
pub use tracing::info;
pub use tracing::trace;
pub use tracing::warn;
use tracing::Level;
use tracing_log::log::LevelFilter;
use tracing_subscriber::fmt::time::OffsetTime;

static mut OFFSET: Option<UtcOffset> = None;

pub fn init_offset() {
    let offset: UtcOffset = UtcOffset::current_local_offset().unwrap();
    unsafe { OFFSET = Some(offset) };
}

pub fn init_logger(level: Level) -> anyhow::Result<()> {
    let level = match level {
        Level::TRACE => "trace",
        Level::DEBUG => "debug",
        Level::INFO => "info",
        Level::WARN => "warn",
        Level::ERROR => "error",
    };

    let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let offset = unsafe {
        if let Some(offset) = OFFSET {
            offset
        } else {
            time::UtcOffset::UTC
        }
    };
    let timer = OffsetTime::new(offset, format);
    let builder = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(format!(
            "{level},h2=off,hyper=off,tokio_util=off,tower_http=debug,refinery_core=warn"
        ))
        .with_writer(std::io::stderr)
        .with_timer(timer);

    #[cfg(target_os = "windows")]
    let builder = builder.with_ansi(false);

    Ok(tracing::subscriber::set_global_default(builder.finish())?)
}

pub fn init_stdlog() {
    tracing_log::LogTracer::builder()
        .with_max_level(LevelFilter::Info)
        .init()
        .unwrap();
}
