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

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use rusqlite::params;

use crate::logger::log;

const DELAY: u64 = 60;
const LIMIT: u64 = 1000;

pub struct RetentionConfig {
    pub days: u64,
}

pub fn retention_task(config: RetentionConfig, conn: Arc<Mutex<rusqlite::Connection>>) {
    let default_delay = Duration::from_secs(DELAY);
    let report_interval = Duration::from_secs(60);

    // Delay on startup.
    std::thread::sleep(default_delay);

    let mut last_report = Instant::now();
    let mut count: u64 = 0;

    loop {
        let mut delay = default_delay;
        match do_retention(&config, conn.clone()) {
            Ok(n) => {
                if n == LIMIT {
                    delay = Duration::from_secs(1);
                }
                count += n;
            }
            Err(err) => {
                log::error!("Database retention job failed: {}", err);
            }
        }
        if last_report.elapsed() > report_interval {
            log::debug!("Events purged in last {:?}: {}", report_interval, count);
            count = 0;
            last_report = Instant::now();
        }
        std::thread::sleep(delay);
    }
}

fn do_retention(config: &RetentionConfig, conn: Arc<Mutex<rusqlite::Connection>>) -> Result<u64> {
    let now = chrono::Utc::now();
    let period = chrono::Duration::from_std(Duration::from_secs(config.days * 86400)).unwrap();
    let older_than = now.checked_sub_signed(period).unwrap();
    let mut conn = conn.lock().unwrap();
    let tx = conn.transaction()?;
    let sql = r#"DELETE FROM events
                WHERE rowid IN
                    (SELECT rowid FROM events WHERE timestamp < ? and escalated = 0 LIMIT ?)"#;
    let n = tx.execute(sql, params![older_than.timestamp_nanos(), LIMIT as i64])?;
    tx.commit()?;
    Ok(n as u64)
}
