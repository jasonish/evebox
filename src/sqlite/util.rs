// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use sqlx::{prelude::FromRow, SqliteConnection, SqliteExecutor};
use tracing::info;

use crate::sqlite::importer::extract_values;

use super::has_table;

pub(crate) async fn fts_create(tx: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE VIRTUAL TABLE fts
             USING fts5(timestamp unindexed, source_values, content=events, content_rowid=rowid)",
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "
        CREATE TRIGGER events_ad_trigger AFTER DELETE ON events BEGIN
          INSERT INTO fts(fts, rowid, timestamp, source_values)
            VALUES ('delete', old.rowid, old.timestamp, old.source_values);
        END",
    )
    .execute(&mut *tx)
    .await?;

    Ok(())
}

pub(crate) async fn fts_check(conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO fts(fts, rank) VALUES ('integrity-check', 1)")
        .execute(conn)
        .await?;
    Ok(())
}

pub(crate) async fn enable_auto_vacuum<'a>(
    conn: impl SqliteExecutor<'a>,
) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA auto_vacuum = 1; VACUUM")
        .execute(conn)
        .await?;
    Ok(())
}

pub(crate) async fn fts_enable(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    if has_table(&mut *conn, "fts").await? {
        return Ok(());
    }
    fts_create(&mut *conn).await?;
    reindex_fts(&mut *conn).await?;
    Ok(())
}

pub(crate) async fn fts_disable(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    sqlx::query("DROP TABLE fts").execute(&mut *conn).await?;
    sqlx::query("DROP TRIGGER events_ad_trigger")
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn reindex_fts(conn: &mut SqliteConnection) -> anyhow::Result<usize> {
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
            let source: serde_json::Value = serde_json::from_str(&row.source)?;
            let flat = extract_values(&source);

            let sql = r#"
                UPDATE events SET source_values = ? WHERE rowid = ?;
                INSERT INTO fts (rowid, timestamp, source_values) VALUES (?, ?, ?)"#;
            sqlx::query(sql)
                .bind(&flat)
                .bind(row.rowid)
                .bind(row.rowid)
                .bind(row.timestamp)
                .bind(&flat)
                .execute(&mut *conn)
                .await?;

            next_id = row.rowid + 1;
            count += 1;
        }

        info!("FTS: Indexed {} events", count);
    }

    Ok(count)
}
