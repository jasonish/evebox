// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

use crate::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use rusqlite::params;

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
                error!("Database retention job failed: {}", err);
            }
        }
        if last_report.elapsed() > report_interval {
            debug!("Events purged in last {:?}: {}", report_interval, count);
            count = 0;
            last_report = Instant::now();
        }
        std::thread::sleep(delay);
    }
}

fn do_retention(config: &RetentionConfig, conn: Arc<Mutex<rusqlite::Connection>>) -> Result<u64> {
    use core::ops::Sub;
    let now = time::OffsetDateTime::now_utc();
    let period = std::time::Duration::from_secs(config.days * 86400);
    let older_than = now.sub(period);
    let mut conn = conn.lock().unwrap();
    let tx = conn.transaction()?;
    let sql = r#"DELETE FROM events
                WHERE rowid IN
                    (SELECT rowid FROM events WHERE timestamp < ? and escalated = 0 LIMIT ?)"#;
    let n = tx.execute(
        sql,
        params![older_than.unix_timestamp_nanos() as i64, LIMIT as i64],
    )?;
    tx.commit()?;
    Ok(n as u64)
}
