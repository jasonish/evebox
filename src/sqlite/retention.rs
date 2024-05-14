// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use core::ops::Sub;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, trace, warn};

use super::info::Infox;
use crate::config::Config;

const DEFAULT_RANGE: usize = 7;

/// How often to run the retention job.  Currently 60 seconds.
const INTERVAL: u64 = 3;

/// The time to sleep between retention runs if not all old events
/// were deleted.
const REPEAT_INTERVAL: u64 = 1;

/// Number of events to delete per run.
const LIMIT: usize = 1000;

#[derive(Debug)]
pub(crate) struct RetentionConfig {
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

pub(crate) async fn start_retention_task(
    config: Config,
    pool: SqlitePool,
    filename: PathBuf,
) -> anyhow::Result<()> {
    let size = get_size(&config)
        .map_err(|err| anyhow::anyhow!("Bad database.retention.size: {:?}", err))?;
    let range = get_days(&config)?;
    info!(
        "Database retention settings: days={}, size={}",
        range.unwrap_or(0),
        size
    );
    let config = RetentionConfig { range, size };
    tokio::spawn(async move {
        retention_task(config, pool, filename).await;
    });

    Ok(())
}

async fn size_enabled(conn: &SqlitePool) -> bool {
    let mut tx = conn.begin().await.unwrap();
    match Infox::new(&mut tx).get_auto_vacuum().await {
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

async fn retention_task(config: RetentionConfig, pool: SqlitePool, filename: PathBuf) {
    let size_enabled = size_enabled(&pool).await;
    let default_delay = Duration::from_secs(INTERVAL);
    let report_interval = Duration::from_secs(60);

    // Delay on startup.
    tokio::time::sleep(default_delay).await;

    let mut last_report = Instant::now();
    let mut count: u64 = 0;

    loop {
        trace!("Running retention task");
        let mut delay = default_delay;

        if size_enabled && config.size > 0 {
            match delete_to_size(&pool, &filename, config.size).await {
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
                match delete_by_range(&pool, range as u64, LIMIT as u64).await {
                    Ok(n) => {
                        count += n;
                        if n == LIMIT as u64 {
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

async fn delete_to_size(conn: &SqlitePool, filename: &Path, bytes: usize) -> Result<u64> {
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
        deleted += delete_events(conn, 1000).await?;
        std::thread::sleep(Duration::from_millis(100));
    }
}

async fn delete_by_range(conn: &SqlitePool, range: u64, limit: u64) -> Result<u64> {
    let now = time::OffsetDateTime::now_utc();
    let period = std::time::Duration::from_secs(range * 86400);
    let older_than = now.sub(period);
    let timer = Instant::now();
    trace!("Deleting events older than {range} days");
    let sql = r#"DELETE FROM events
        WHERE rowid IN
            (SELECT rowid FROM
             events WHERE timestamp < ? 
               AND escalated = 0 
             ORDER BY timestamp ASC
             LIMIT ?)"#;
    let n = sqlx::query(sql)
        .bind(older_than.unix_timestamp_nanos() as i64)
        .bind(limit as i64)
        .execute(conn)
        .await?
        .rows_affected();
    if n > 0 {
        debug!(
            "Deleted {n} events older than {} ({range} days) in {} ms",
            &older_than,
            timer.elapsed().as_millis()
        );
    }
    Ok(n)
}

async fn delete_events(conn: &SqlitePool, limit: usize) -> Result<u64> {
    let timer = Instant::now();
    let sql = r#"DELETE FROM events
        WHERE rowid IN
            (SELECT rowid 
             FROM events
             WHERE escalated = 0
             ORDER BY timestamp ASC
             LIMIT ?)"#;
    let n = sqlx::query(sql)
        .bind(limit as i64)
        .execute(conn)
        .await?
        .rows_affected();
    trace!("Deleted {n} events in {} ms", timer.elapsed().as_millis());
    Ok(n)
}
