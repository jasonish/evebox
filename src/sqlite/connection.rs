// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::{
    prelude::*,
    sqlite::{util::fts_create, SqliteExt},
};
use rusqlite::{params, Connection, DatabaseName, OpenFlags};
use std::path::PathBuf;
use time::format_description::well_known::Rfc3339;

pub struct ConnectionBuilder {
    pub filename: Option<PathBuf>,
}

impl ConnectionBuilder {
    pub fn filename<T: Into<PathBuf>>(filename: Option<T>) -> ConnectionBuilder {
        ConnectionBuilder {
            filename: filename.map(|filename| filename.into()),
        }
    }

    pub fn open(&self, create: bool) -> Result<Connection, rusqlite::Error> {
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        if let Some(filename) = &self.filename {
            debug!("Opening database {}", filename.display());
            rusqlite::Connection::open_with_flags(filename, flags)
        } else {
            rusqlite::Connection::open_in_memory()
        }
    }
}

pub fn init_event_db(db: &mut Connection) -> Result<(), rusqlite::Error> {
    let auto_vacuum = get_auto_vacuum(db)?;
    if auto_vacuum == 2 {
        info!("Change auto-vacuum from incremental to full");
        enable_auto_vacuum(db)?;
    } else if auto_vacuum == 0 {
        info!("Attempting to enable auto-vacuum");
        enable_auto_vacuum(db)?;
    }
    let auto_vacuum = get_auto_vacuum(db)?;
    info!("Auto-vacuum: {auto_vacuum}");

    // Set WAL mode.
    let mode = db.pragma_update_and_check(None, "journal_mode", "WAL", |row| {
        let mode: String = row.get(0)?;
        Ok(mode)
    });
    debug!("Result of setting database to WAL mode: {:?}", mode);

    // Set synchronous to NORMAL.
    if let Err(err) = db.pragma_update(None, "synchronous", "NORMAL") {
        error!("Failed to set pragma synchronous = NORMAL: {:?}", err);
    }
    match db.pragma_query_value(None, "synchronous", |row| {
        let val: i32 = row.get(0)?;
        Ok(val)
    }) {
        Ok(mode) => {
            if mode != 1 {
                warn!("Database not in synchronous mode normal, instead: {}", mode);
            }
        }
        Err(err) => {
            warn!("Failed to query pragma synchronous: {:?}", err);
        }
    }

    // This will only be a value if we have a database from before the
    // use of refinery.
    let version = db
        .query_row("select max(version) from schema", [], |row| {
            let version: i64 = row.get(0).unwrap();
            Ok(version)
        })
        .unwrap_or(-1);

    if version > -1 && version <= 3 {
        // We may have to provide the refinery table, unless it was already created.
        debug!("SQLite configuration DB at v1, checking if setup required for Refinery migrations");
        let fake_refinery_setup = "CREATE TABLE refinery_schema_history(
            version INT4 PRIMARY KEY,
            name VARCHAR(255),
            applied_on VARCHAR(255),
            checksum VARCHAR(255))";
        if db.execute(fake_refinery_setup, []).is_ok() {
            let now = time::OffsetDateTime::now_utc();

            // 1|Initial|2021-10-11T23:13:56.840335347-06:00|13384621929958573416
            // 2|Indices|2021-10-11T23:13:56.841740878-06:00|18013925364710952777
            // 3|RemoveFTS|2021-10-11T23:13:56.842433252-06:00|16609115521065592815

            let formatted_now = now.format(&Rfc3339).unwrap();

            if version > 0 {
                let params = params![1, "Initial", &formatted_now, "13384621929958573416"];
                db.execute(
                    "INSERT INTO refinery_schema_history VALUES (?, ?, ?, ?)",
                    params,
                )?;
            }
            if version > 1 {
                let params = params![2, "Indices", &formatted_now, "18013925364710952777"];
                db.execute(
                    "INSERT INTO refinery_schema_history VALUES (?, ?, ?, ?)",
                    params,
                )?;
            }
            if version > 2 {
                let params = params![3, "RemoveFTS", &formatted_now, "16609115521065592815"];
                db.execute(
                    "INSERT INTO refinery_schema_history VALUES (?, ?, ?, ?)",
                    params,
                )?;
            }
        } else {
            debug!("Refinery migrations already exist for SQLite configuration DB");
        }
    }

    let fresh_install = !db.has_table("events")?;

    embedded::migrations::runner().run(db).unwrap();

    if let Some(indexes) = crate::resource::get_string("sqlite/Indexes.sql") {
        info!("Updating SQLite indexes");
        if let Err(err) = db.execute_batch(&indexes) {
            error!("Failed to update SQLite indexes: {err}");
        }
    }

    if fresh_install {
        info!("Enabling FTS");
        let tx = db.transaction()?;
        fts_create(&tx)?;
        tx.commit()?;
    } else if !db.has_table("fts")? {
        info!("FTS not enabled, consider enabling for query performance improvements");
    }

    Ok(())
}

pub(crate) fn get_auto_vacuum(db: &Connection) -> Result<u8, rusqlite::Error> {
    db.query_row_and_then("SELECT auto_vacuum FROM pragma_auto_vacuum", [], |row| {
        row.get(0)
    })
}

fn enable_auto_vacuum(db: &Connection) -> Result<(), rusqlite::Error> {
    db.pragma_update(Some(DatabaseName::Main), "auto_vacuum", 1)
}

pub(crate) fn get_journal_mode(db: &Connection) -> Result<String, rusqlite::Error> {
    db.query_row_and_then("SELECT journal_mode FROM pragma_journal_mode", [], |row| {
        row.get(0)
    })
}

pub(crate) fn get_synchronous(db: &Connection) -> Result<u8, rusqlite::Error> {
    db.query_row_and_then("SELECT synchronous FROM pragma_synchronous", [], |row| {
        row.get(0)
    })
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./resources/sqlite/migrations");
}
