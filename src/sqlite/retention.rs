// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::config::Config;
use crate::prelude::*;
use anyhow::Result;
use core::ops::Sub;
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::connection::get_auto_vacuum;

const DEFAULT_RANGE: usize = 7;

/// How often to run the retention job.  Currently 60 seconds.
const INTERVAL: u64 = 3;

/// The time to sleep between retention runs if not all old events
/// were deleted.
const REPEAT_INTERVAL: u64 = 1;

/// Number of events to delete per run.
const LIMIT: usize = 1000;

#[derive(Debug)]
pub struct RetentionConfig {
    pub range: Option<usize>,
    pub size: usize,
}

fn get_size(config: &Config) -> Result<usize> {
    // Size as a number.
    if let Ok(Some(size)) = config.get::<usize>("database.retention.size") {
        Ok(size)
    } else if let Ok(Some(size)) = config.get::<String>("database.retention.size") {
        if let Ok(size) = size.parse::<usize>() {
            Ok(size)
        } else {
            crate::util::parse_humansize(&size)
        }
    } else {
        Ok(0)
    }
}

fn get_days(config: &Config) -> Result<Option<usize>> {
    let days = if let Some(days) = config.get::<usize>("database.retention.days")? {
        days
    } else if let Some(days) = config.get::<usize>("database.retention-period")? {
        days
    } else {
        DEFAULT_RANGE
    };
    if days > 0 {
        Ok(Some(days))
    } else {
        Ok(None)
    }
}

pub fn start_retention_task(config: Config, conn: Arc<Mutex<Connection>>) -> anyhow::Result<()> {
    let size = get_size(&config)
        .map_err(|err| anyhow::anyhow!("Bad database.retention.size: {:?}", err))?;
    let range = get_days(&config)?;
    info!(
        "Database retention settings: days={}, size={}",
        range.unwrap_or(0),
        size
    );
    let config = RetentionConfig { range, size };
    tokio::task::spawn_blocking(|| {
        retention_task(config, conn);
    });
    Ok(())
}

fn size_enabled(conn: &Arc<Mutex<Connection>>) -> bool {
    let conn = conn.lock().unwrap();
    match get_auto_vacuum(&conn) {
        Ok(mode) => {
            if mode == 0 {
                warn!("Auto-vacuum not available, size based retention not available");
                false
            } else if mode == 1 {
                debug!("Auto-vacuum in mode full, size based retention available");
                true
            } else if mode == 2 {
                warn!("Auto-vacuum in incremental mode, size based retention not available");
                false
            } else {
                error!("Unknown auto-vacuum mode {mode}, size based retention not available");
                false
            }
        }
        Err(err) => {
            error!(
                "Failed to get auto-vacuum mode, sized based retention not available: {:?}",
                err
            );
            false
        }
    }
}

fn retention_task(config: RetentionConfig, conn: Arc<Mutex<rusqlite::Connection>>) {
    let size_enabled = size_enabled(&conn);
    let default_delay = Duration::from_secs(INTERVAL);
    let report_interval = Duration::from_secs(60);
    let filename = conn
        .lock()
        .map(|conn| conn.path().map(|p| p.to_string()))
        .unwrap();

    // Delay on startup.
    std::thread::sleep(default_delay);

    let mut last_report = Instant::now();
    let mut count: usize = 0;

    loop {
        let mut delay = default_delay;

        if filename.is_some() && size_enabled && config.size > 0 {
            match delete_to_size(&conn, filename.as_ref().unwrap(), config.size) {
                Err(err) => {
                    error!("Failed to delete database to max size: {:?}", err);
                }
                Ok(n) => {
                    if n > 0 {
                        info!(
                            "Deleted {n} events to reduce database size to {} bytes",
                            config.size
                        );
                        count += n;
                    }
                }
            }
        }

        // Range (day) based retention.
        if let Some(range) = config.range {
            if range > 0 {
                match delete_by_range(&conn, range, LIMIT) {
                    Ok(n) => {
                        count += n;
                        if n == LIMIT {
                            delay = Duration::from_secs(REPEAT_INTERVAL);
                        }
                    }
                    Err(err) => {
                        error!("Database retention job failed: {}", err);
                    }
                }
            }
        }

        if last_report.elapsed() > report_interval {
            info!("Events purged in last {:?}: {}", report_interval, count);
            count = 0;
            last_report = Instant::now();
        }
        std::thread::sleep(delay);
    }
}

fn delete_to_size(conn: &Arc<Mutex<Connection>>, filename: &str, bytes: usize) -> Result<usize> {
    let file_size = crate::file::file_size(filename)? as usize;
    if file_size < bytes {
        trace!("Database less than max size of {} bytes", bytes);
        return Ok(0);
    }

    let mut deleted = 0;
    loop {
        let file_size = crate::file::file_size(filename)? as usize;
        if file_size < bytes {
            return Ok(deleted);
        }

        trace!("Database file size of {} bytes is greater than max allowed size of {} bytes, deleting events",
	       file_size, bytes);
        deleted += delete_events(conn, 1000)?;
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn delete_by_range(conn: &Arc<Mutex<Connection>>, range: usize, limit: usize) -> Result<usize> {
    let now = time::OffsetDateTime::now_utc();
    let period = std::time::Duration::from_secs(range as u64 * 86400);
    let older_than = now.sub(period);
    let mut conn = conn.lock().unwrap();
    let timer = Instant::now();
    trace!("Deleting events older than {range} days");
    let tx = conn.transaction()?;
    let sql = r#"DELETE FROM events
                WHERE rowid IN
                    (SELECT rowid FROM events WHERE timestamp < ? and escalated = 0 ORDER BY timestamp ASC LIMIT ?)"#;
    let n = tx.execute(
        sql,
        params![older_than.unix_timestamp_nanos() as i64, limit as i64],
    )?;
    tx.commit()?;
    if n > 0 {
        debug!(
            "Deleted {n} events older than {} ({range} days) in {} ms",
            &older_than,
            timer.elapsed().as_millis()
        );
    }
    Ok(n)
}

fn delete_events(conn: &Arc<Mutex<rusqlite::Connection>>, limit: usize) -> Result<usize> {
    let sql = "delete from events where rowid in (select rowid from events where escalated = 0 order by timestamp asc limit ?)";
    let conn = conn.lock().unwrap();
    let timer = Instant::now();
    let mut st = conn.prepare(sql)?;
    let n = st.execute(params![limit])?;
    trace!("Deleted {n} events in {} ms", timer.elapsed().as_millis());
    Ok(n)
}
