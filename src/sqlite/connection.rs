// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::prelude::*;
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

    pub fn open(&self) -> Result<Connection, rusqlite::Error> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_SHARED_CACHE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX;
        if let Some(filename) = &self.filename {
            rusqlite::Connection::open_with_flags(filename, flags)
        } else {
            rusqlite::Connection::open_in_memory()
        }
    }
}

pub fn init_event_db(db: &mut Connection) -> Result<(), rusqlite::Error> {
    let auto_vacuum = get_auto_vacuum(db)?;
    if auto_vacuum == 0 {
        enable_auto_vacuum(db)?;
        if get_auto_vacuum(db)? == 0 {
            info!("Auto-vacuum not enabled");
        }
    }

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

    embedded::migrations::runner().run(db).unwrap();
    Ok(())
}

fn get_auto_vacuum(db: &Connection) -> Result<u8, rusqlite::Error> {
    db.query_row_and_then("SELECT auto_vacuum FROM pragma_auto_vacuum", [], |row| {
        row.get(0)
    })
}

fn enable_auto_vacuum(db: &Connection) -> Result<(), rusqlite::Error> {
    db.pragma_update(Some(DatabaseName::Main), "auto_vacuum", 2)
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./resources/sqlite/migrations");
}