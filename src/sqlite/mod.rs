// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

pub mod configrepo;
pub mod connection;
pub mod eventstore;
pub mod importer;
pub mod retention;
pub mod builder;

use crate::prelude::*;
pub use connection::init_event_db;
pub use connection::ConnectionBuilder;
use std::path::PathBuf;
use time::macros::format_description;

pub async fn open_pool<T: Into<PathBuf>>(filename: T) -> anyhow::Result<deadpool_sqlite::Pool> {
    use deadpool_sqlite::{Config, Runtime};
    let config = Config::new(filename);
    let pool = config.create_pool(Runtime::Tokio1)?;
    let conn = pool.get().await?;
    if let Err(err) = conn
        .interact(|conn| {
            debug!("set journal mode to WAL");
            let mode = conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| {
                let mode: String = row.get(0)?;
                Ok(mode)
            });
            info!("Result of setting database to WAL mode: {:?}", mode);

            // Set synchronous to NORMAL.
            if let Err(err) = conn.pragma_update(None, "synchronous", "NORMAL") {
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

pub fn format_sqlite_timestamp(dt: &time::OffsetDateTime) -> String {
    let format =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6][offset_hour sign:mandatory][offset_minute]");
    dt.to_offset(time::UtcOffset::UTC).format(&format).unwrap()
}
