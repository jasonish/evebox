// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use rusqlite::Transaction;

pub fn fts_create(tx: &Transaction) -> Result<(), rusqlite::Error> {
    tx.execute(
        "create virtual table fts
            using fts5(timestamp unindexed, source_values, content=events, content_rowid=rowid)",
        [],
    )?;
    tx.execute(
        "
        create trigger events_ad_trigger after delete on events begin
          insert into fts(fts, rowid, timestamp, source_values)
            values ('delete', old.rowid, old.timestamp, old.source_values);
        end",
        [],
    )?;

    Ok(())
}
