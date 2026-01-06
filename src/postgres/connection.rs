// SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tracing::{debug, info};

/// Open a connection pool to PostgreSQL.
pub(crate) async fn open_pool(url: &str) -> Result<PgPool, sqlx::Error> {
    use sqlx::ConnectOptions;
    use std::time::Duration;

    info!("Connecting to PostgreSQL");

    let mut options: PgConnectOptions = url.parse()?;

    // Set slow statement threshold to 3 seconds
    options = options.log_slow_statements(log::LevelFilter::Warn, Duration::from_secs(3));

    let pool = PgPoolOptions::new()
        .min_connections(8)
        .max_connections(32)
        .connect_with(options)
        .await?;

    info!("Connected to PostgreSQL");
    Ok(pool)
}

/// Initialize the event database by running migrations.
pub(crate) async fn init_event_db(pool: &PgPool) -> anyhow::Result<()> {
    info!("Running PostgreSQL migrations");

    sqlx::migrate!("resources/postgres/migrations")
        .run(pool)
        .await?;

    debug!("PostgreSQL migrations complete");
    Ok(())
}
