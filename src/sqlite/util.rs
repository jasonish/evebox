// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use sqlx::{SqliteConnection, SqliteExecutor};

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
