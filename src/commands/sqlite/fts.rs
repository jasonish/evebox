// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::{FtsArgs, FtsCommand};
use crate::sqlite::{
    connection::init_event_db2,
    has_table,
    importer::extract_values,
    util::{self, fts_create},
    ConnectionBuilder,
};
use anyhow::Result;
use serde_json::Value;
use sqlx::SqliteConnection;
use sqlx::{Connection, FromRow};
use tracing::{debug, info, warn};

pub(super) async fn fts(args: &FtsArgs) -> Result<()> {
    match &args.command {
        FtsCommand::Enable { force, filename } => fts_enable(force, filename).await,
        FtsCommand::Disable { force, filename } => fts_disable(force, filename).await,
        FtsCommand::Check { filename } => fts_check(filename).await,
        FtsCommand::Optimize { filename } => fts_optimize(filename).await,
    }
}

async fn fts_disable(force: &bool, filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_sqlx_connection(false)
        .await?;
    if !has_table(&mut conn, "fts").await? {
        warn!("FTS not enabled");
    } else {
        if !force {
            bail!("Please make sure EveBox is NOT running then re-run with --force");
        }
        info!("Disabling FTS, this could take a while");
        let mut tx = conn.begin().await?;
        sqlx::query("DROP TABLE fts").execute(&mut *tx).await?;
        sqlx::query("DROP TRIGGER events_ad_trigger")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        info!("FTS has been disabled");
    }
    Ok(())
}

async fn fts_enable(force: &bool, filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_sqlx_connection(false)
        .await?;
    if has_table(&mut conn, "fts").await? {
        bail!("FTS is already enabled");
    }

    if !force {
        bail!("Please make sure EveBox is NOT running then re-run with --force");
    }

    init_event_db2(&mut conn).await?;
    let mut tx = conn.begin().await?;
    fts_create(&mut tx).await?;

    info!("Building FTS index, this could take a while");

    let count = reindex_fts(&mut tx).await?;

    tx.commit().await?;

    info!("Indexed {count} events");

    Ok(())
}

async fn fts_check(filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_sqlx_connection(false)
        .await?;
    if !has_table(&mut conn, "fts").await? {
        warn!("FTS is not enabled");
        return Ok(());
    }
    info!("FTS is enabled, checking integrity");

    match util::fts_check(&mut conn).await {
        Ok(_) => {
            info!("FTS data OK");
        }
        Err(err) => {
            bail!("FTS data is NOT OK: {:?}", err);
        }
    }

    Ok(())
}

async fn get_total_changes(conn: &mut SqliteConnection) -> Result<i64> {
    let count: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(&mut *conn)
        .await?;
    Ok(count)
}

async fn fts_optimize(filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_sqlx_connection(false)
        .await?;

    if !has_table(&mut conn, "fts").await? {
        warn!("FTS is not enabled");
        return Ok(());
    }

    info!("Running SQLite FTS optimization");

    let mut last_total_changes = get_total_changes(&mut conn).await?;

    sqlx::query("INSERT INTO fts(fts, rank) VALUES ('merge', -500)")
        .execute(&mut conn)
        .await?;

    loop {
        sqlx::query("INSERT INTO fts(fts, rank) VALUES ('merge', 500)")
            .execute(&mut conn)
            .await?;
        let total_changes = get_total_changes(&mut conn).await?;
        let changes = total_changes - last_total_changes;
        debug!("Modified rows: {changes}");
        if changes < 2 {
            break;
        }
        last_total_changes = total_changes;
    }

    Ok(())
}

async fn reindex_fts(conn: &mut SqliteConnection) -> Result<usize> {
    let mut next_id = 0;
    let mut count = 0;

    #[derive(FromRow)]
    struct EventRow {
        rowid: i64,
        timestamp: i64,
        source: String,
    }

    loop {
        let rows: Vec<EventRow> = sqlx::query_as(
            "SELECT rowid, timestamp, source FROM events WHERE rowid >= ? ORDER BY rowid ASC LIMIT 10000",
        )
            .bind(next_id)
            .fetch_all(&mut *conn)
            .await?;
        if rows.is_empty() {
            break;
        }

        for row in rows {
            let source: Value = serde_json::from_str(&row.source)?;
            let flat = extract_values(&source);
            sqlx::query("UPDATE events SET source_values = ? WHERE rowid = ?")
                .bind(&flat)
                .bind(row.rowid)
                .execute(&mut *conn)
                .await?;
            sqlx::query("INSERT INTO fts (rowid, timestamp, source_values) VALUES (?, ?, ?)")
                .bind(row.rowid)
                .bind(row.timestamp)
                .bind(&flat)
                .execute(&mut *conn)
                .await?;
            next_id = row.rowid + 1;
            count += 1;
        }

        info!("{}", count);
    }

    Ok(count)
}
