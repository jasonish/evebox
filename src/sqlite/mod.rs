// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use rusqlite::params;
use rusqlite::OpenFlags;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;

use crate::prelude::*;

pub mod configrepo;
pub mod eventstore;
pub mod importer;
pub mod retention;

pub async fn open_pool<T: Into<PathBuf>>(filename: T) -> anyhow::Result<deadpool_sqlite::Pool> {
    use deadpool_sqlite::{Config, Runtime};
    let config = Config::new(filename);
    let pool = config.create_pool(Runtime::Tokio1)?;
    let conn = pool.get().await?;
    if let Err(err) = conn
        .interact(|conn| {
            debug!("set journal mode to WAL");
            let mode = conn.pragma_update_and_check(None, "journal_mode", &"WAL", |row| {
                let mode: String = row.get(0)?;
                Ok(mode)
            });
            info!("Result of setting database to WAL mode: {:?}", mode);

            // Set synchronous to NORMAL.
            if let Err(err) = conn.pragma_update(None, "synchronous", &"NORMAL") {
                error!("Failed to set pragma synchronous = NORMAL: {:?}", err);
            }
            match conn.pragma_query_value(None, "synchronous", |row| {
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
        })
        .await
    {
        return Err(anyhow::anyhow!("{:?}", err));
    }
    Ok(pool)
}

pub struct ConnectionBuilder {
    pub filename: Option<PathBuf>,
}

impl ConnectionBuilder {
    pub fn filename<T: Into<PathBuf>>(filename: Option<T>) -> ConnectionBuilder {
        ConnectionBuilder {
            filename: filename.map(|filename| filename.into()),
        }
    }

    pub fn open(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_SHARED_CACHE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX;
        if let Some(filename) = &self.filename {
            let conn = rusqlite::Connection::open_with_flags(filename, flags)?;

            // Set WAL mode.
            let mode = conn.pragma_update_and_check(None, "journal_mode", &"WAL", |row| {
                let mode: String = row.get(0)?;
                Ok(mode)
            });
            debug!("Result of setting database to WAL mode: {:?}", mode);

            // Set synchronous to NORMAL.
            if let Err(err) = conn.pragma_update(None, "synchronous", &"NORMAL") {
                error!("Failed to set pragma synchronous = NORMAL: {:?}", err);
            }
            match conn.pragma_query_value(None, "synchronous", |row| {
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

            Ok(conn)
        } else {
            rusqlite::Connection::open_in_memory()
        }
    }
}

pub fn init_event_db(db: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let version = db
        .query_row("select max(version) from schema", params![], |row| {
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
        if db.execute(fake_refinery_setup, params![]).is_ok() {
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

pub fn format_sqlite_timestamp(dt: &time::OffsetDateTime) -> String {
    let format =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6][offset_hour sign:mandatory][offset_minute]");
    dt.to_offset(time::UtcOffset::UTC).format(&format).unwrap()
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./resources/sqlite/migrations");
}
