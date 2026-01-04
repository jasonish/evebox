// SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tracing::{debug, info};

/// Open a connection pool to PostgreSQL.
pub(crate) async fn open_pool(url: &str) -> Result<PgPool, sqlx::Error> {
    info!("Connecting to PostgreSQL");

    let pool = PgPoolOptions::new()
        .min_connections(2)
        .max_connections(10)
        .connect(url)
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
