// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use sqlx::SqliteConnection;

pub(crate) async fn fts_create(tx: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::query(
        "create virtual table fts
            using fts5(timestamp unindexed, source_values, content=events, content_rowid=rowid)",
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "
        create trigger events_ad_trigger after delete on events begin
          insert into fts(fts, rowid, timestamp, source_values)
            values ('delete', old.rowid, old.timestamp, old.source_values);
        end",
    )
    .execute(&mut *tx)
    .await?;

    Ok(())
}

pub(crate) fn fts_check_rusqlite(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("insert into fts(fts, rank) values ('integrity-check', 1)")?;
    let _ = stmt.execute([])?;
    Ok(())
}

pub(crate) async fn fts_check(conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO fts(fts, rank) VALUES ('integrity-check', 1)")
        .execute(conn)
        .await?;
    Ok(())
}
