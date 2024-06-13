// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::{FtsArgs, FtsCommand};
use crate::sqlite::{connection::init_event_db, has_table, util, ConnectionBuilder};
use anyhow::Result;
use owo_colors::OwoColorize;
use sqlx::{Connection, SqliteConnection};
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
        .open_connection(false)
        .await?;
    if !has_table(&mut conn, "fts").await? {
        warn!("FTS not enabled");
    } else {
        if !force {
            let ex = "!".cyan();
            println!(
                r#"{ex} Notice:
{ex}
{ex} While disabling FTS is rather quick, re-enabling FTS on a large database
{ex} can take a long time, where the database is not available for writes."#
            );
            let ok = inquire::Confirm::new("Do you wish to continue?")
                .prompt()
                .unwrap_or(false);
            if !ok {
                return Ok(());
            }
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
        .open_connection(false)
        .await?;
    if has_table(&mut conn, "fts").await? {
        bail!("FTS is already enabled");
    }

    if !force {
        let ex = "!".cyan();
        println!(
            r#"{ex} Notice:
{ex}
{ex} Enabling FTS on a large database can take a long time. The database will
{ex} not be available for writes during this time."#
        );
        let ok = inquire::Confirm::new("Do you wish to continue?")
            .prompt()
            .unwrap_or(false);
        if !ok {
            return Ok(());
        }
    }

    init_event_db(&mut conn).await?;
    let mut tx = conn.begin().await?;
    crate::sqlite::util::fts_enable(&mut tx).await?;
    tx.commit().await?;

    info!("FTS enabled");

    Ok(())
}

async fn fts_check(filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_connection(false)
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
        .open_connection(false)
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
