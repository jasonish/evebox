// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use time::macros::format_description;
use tracing::Level;
use tracing_log::log::LevelFilter;

pub fn init_logger(level: Level) -> anyhow::Result<()> {
    let level = match level {
        Level::TRACE => "trace",
        Level::DEBUG => "debug",
        Level::INFO => "info",
        Level::WARN => "warn",
        Level::ERROR => "error",
    };

    let is_utc = if let Ok(offset) = time::UtcOffset::current_local_offset() {
        offset == time::UtcOffset::UTC
    } else {
        false
    };

    let format = if is_utc {
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z")
    } else {
        format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory][offset_minute]"
        )
    };

    let timer = tracing_subscriber::fmt::time::LocalTime::new(format);
    let builder = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(format!(
            "{level},h2=off,hyper=off,tokio_util=off,tower_http=debug,refinery_core=warn,sqlx::query=warn"
        ))
        .with_writer(std::io::stderr)
        .with_timer(timer);

    Ok(tracing::subscriber::set_global_default(builder.finish())?)
}

pub fn init_stdlog() {
    tracing_log::LogTracer::builder()
        .with_max_level(LevelFilter::Info)
        .init()
        .unwrap();
}
