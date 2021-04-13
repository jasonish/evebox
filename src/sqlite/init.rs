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

use crate::prelude::*;
use crate::resource::Resource;
use rusqlite::params;

pub fn init_db(db: &mut rusqlite::Connection, prefix: &str) -> Result<(), rusqlite::Error> {
    let version = db
        .query_row("select max(version) from schema", params![], |row| {
            let version: i64 = row.get(0).unwrap();
            Ok(version)
        })
        .unwrap_or(-1);
    info!("Found event database schema version {}", version);
    let mut next_version = version + 1;

    loop {
        let filename = format!("{}/V{}.sql", prefix, next_version);
        if let Some(asset) = Resource::get(&filename) {
            if next_version == 0 {
                info!("Initializing SQLite database ({})", prefix)
            } else {
                info!(
                    "Updating SQLite database to schema version {} ({})",
                    next_version, prefix
                );
            }
            let asset = String::from_utf8_lossy(&asset);
            let tx = db.transaction()?;
            tx.execute_batch(&asset)?;
            tx.execute(
                "INSERT INTO schema (version, timestamp) VALUES (?1, date('now'))",
                params![next_version],
            )?;
            tx.commit()?;
            next_version += 1;
        } else {
            debug!(
                "Did not find resource file {}, database migration done",
                filename
            );
            break;
        }
    }

    Ok(())
}
