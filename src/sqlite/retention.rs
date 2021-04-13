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
