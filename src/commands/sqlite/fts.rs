// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::{FtsArgs, FtsCommand};
use crate::sqlite::{
    importer::extract_values, init_event_db, util::{self, fts_create}, ConnectionBuilder, SqliteExt,
};
use anyhow::Result;
use rusqlite::{params, Transaction};
use serde_json::Value;
use tracing::{debug, info, warn};

pub(super) fn fts(args: &FtsArgs) -> Result<()> {
    match &args.command {
        FtsCommand::Enable { force, filename } => fts_enable(force, filename),
        FtsCommand::Disable { force, filename } => fts_disable(force, filename),
        FtsCommand::Check { filename } => fts_check(filename),
        FtsCommand::Optimize { filename } => fts_optimize(filename),
    }
}

fn fts_disable(force: &bool, filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename)).open(false)?;
    if !conn.has_table("fts")? {
        warn!("FTS not enabled");
    } else {
        if !force {
            bail!("Please make sure EveBox is NOT running then re-run with --force");
        }
        info!("Disabling FTS, this could take a while");
        let tx = conn.transaction()?;
        tx.execute("drop table fts", [])?;
        tx.execute("drop trigger events_ad_trigger", [])?;
        tx.commit()?;
        info!("FTS has been disabled");
    }
    Ok(())
}

fn fts_enable(force: &bool, filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename)).open(false)?;

    if conn.has_table("fts")? {
        bail!("FTS already enabled");
    }

    if !force {
        bail!("Please make sure EveBox is NOT running then re-run with --force");
    }

    init_event_db(&mut conn)?;
    let tx = conn.transaction()?;
    fts_create(&tx)?;

    info!("Building FTS index, this could take a while");

    let count = reindex_fts(&tx)?;

    tx.commit()?;

    info!("Indexed {count} events");

    Ok(())
}

fn fts_check(filename: &str) -> Result<()> {
    let conn = ConnectionBuilder::filename(Some(filename)).open(false)?;
    if !conn.has_table("fts")? {
        warn!("FTS is not enabled");
        return Ok(());
    }
    info!("FTS is enabled, checking integrity");

    match util::fts_check(&conn) {
        Ok(_) => {
            info!("FTS data OK");
        }
        Err(err) => {
            bail!("FTS data is NOT OK: {:?}", err);
        }
    }

    Ok(())
}

fn fts_optimize(filename: &str) -> Result<()> {
    let conn = ConnectionBuilder::filename(Some(filename)).open(false)?;

    if !conn.has_table("fts")? {
        warn!("FTS is not enabled");
        return Ok(());
    }

    let get_total_changes = || -> Result<i64> {
        let rows = conn.query_row("select total_changes()", [], |row| {
            let count: i64 = row.get(0)?;
            Ok(count)
        })?;
        Ok(rows)
    };

    let mut last_total_changes = get_total_changes()?;

    let mut st = conn.prepare("insert into fts(fts, rank) values ('merge', -500)")?;
    st.execute([])?;

    loop {
        conn.execute("insert into fts(fts, rank) values ('merge', 500)", [])?;
        let total_changes = get_total_changes()?;
        let changes = total_changes - last_total_changes;
        debug!("Modified rows: {changes}");
        if changes < 2 {
            break;
        }
        last_total_changes = total_changes;
    }

    Ok(())
}

fn reindex_fts(tx: &Transaction) -> Result<usize> {
    let mut st = tx.prepare("select rowid, timestamp, source from events order by rowid")?;
    let mut rows = st.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let rowid: u64 = row.get(0)?;
        let timestamp: u64 = row.get(1)?;
        let source: String = row.get(2)?;
        let source: Value = serde_json::from_str(&source)?;
        let flat = extract_values(&source);

        tx.execute(
            "update events set source_values = ? where rowid = ?",
            params![&flat, rowid],
        )?;
        tx.execute(
            "insert into fts (rowid, timestamp, source_values) values (?, ?, ?)",
            params![rowid, timestamp, &flat],
        )?;

        count += 1;
    }
    Ok(count)
}
