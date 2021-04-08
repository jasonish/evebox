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
