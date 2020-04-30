// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::path::PathBuf;

use rusqlite::params;
use rusqlite::OpenFlags;

use crate::logger::log;
use crate::resource::Resource;

pub mod configrepo;
pub mod eventstore;
pub mod importer;
pub mod queryparser;
pub mod retention;

pub struct ConnectionBuilder {
    pub filename: Option<PathBuf>,
}

impl ConnectionBuilder {
    pub fn filename<T: Into<PathBuf>>(filename: Option<T>) -> ConnectionBuilder {
        ConnectionBuilder {
            filename: if let Some(filename) = filename {
                Some(filename.into())
            } else {
                None
            },
        }
    }

    pub fn open(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_SHARED_CACHE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX;
        if let Some(filename) = &self.filename {
            let c = rusqlite::Connection::open_with_flags(&filename, flags)?;
            Ok(c)
        } else {
            rusqlite::Connection::open_in_memory()
        }
    }
}

pub fn init_event_db(db: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let version = db.query_row("select max(version) from schema", params![], |row| {
        let version: i64 = row.get(0).unwrap();
        Ok(version)
    });
    let mut version = match version {
        Ok(version) => version + 1,
        Err(_) => 0,
    };

    loop {
        let filename = format!("sqlite/V{}.sql", version);
        let asset = Resource::get(&filename);
        if let Some(asset) = asset {
            if version == 0 {
                log::info!("Initializing SQLite database")
            } else {
                log::info!("Updating SQLite database to schema version {}", version);
            }
            let asset = String::from_utf8_lossy(&asset);
            let tx = db.transaction()?;
            tx.execute_batch(&asset)?;
            tx.execute(
                "INSERT INTO schema (version, timestamp) VALUES (?1, date('now'))",
                params![version],
            )?;
            tx.commit()?;
        } else {
            break;
        }
        version += 1;
    }

    Ok(())
}

/// Format a DateTime object into the SQLite format.
pub fn format_sqlite_timestamp(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let dt = dt.with_timezone(&chrono::Utc);
    dt.format("%Y-%m-%dT%H:%M:%S.%6f%z").to_string()
}
