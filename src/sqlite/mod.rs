// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::path::PathBuf;

use rusqlite::OpenFlags;

pub mod configrepo;
pub mod eventstore;
pub mod importer;
pub mod init;
pub mod queryparser;
pub mod retention;

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
            Ok(rusqlite::Connection::open_with_flags(&filename, flags)?)
        } else {
            rusqlite::Connection::open_in_memory()
        }
    }
}

pub fn init_event_db(db: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    crate::sqlite::init::init_db(db, "sqlite")
}

/// Format a DateTime object into the SQLite format.
pub fn format_sqlite_timestamp(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let dt = dt.with_timezone(&chrono::Utc);
    dt.format("%Y-%m-%dT%H:%M:%S.%6f%z").to_string()
}
