// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub mod builder;
pub mod configrepo;
pub mod connection;
pub mod eventrepo;
pub mod importer;
pub(crate) mod info;
pub mod pool;
pub mod retention;
pub mod util;

pub use connection::init_event_db;
pub use connection::ConnectionBuilder;
use rusqlite::params;
use rusqlite::OptionalExtension;
use time::macros::format_description;

pub fn format_sqlite_timestamp(dt: &time::OffsetDateTime) -> String {
    let format =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6][offset_hour sign:mandatory][offset_minute]");
    dt.to_offset(time::UtcOffset::UTC).format(&format).unwrap()
}

pub trait SqliteExt {
    fn has_table(&self, name: &str) -> Result<bool, rusqlite::Error>;
}

impl SqliteExt for rusqlite::Connection {
    fn has_table(&self, name: &str) -> Result<bool, rusqlite::Error> {
        let row = self
            .query_row(
                "select name from sqlite_master where type = 'table' and name = ?",
                params![name],
                |_| Ok(()),
            )
            .optional()?;
        Ok(row.is_some())
    }
}
