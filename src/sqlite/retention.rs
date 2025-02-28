// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use crate::elastic::HistoryEntryBuilder;
use crate::server::metrics::Metrics;
use crate::sqlite::prelude::*;

use anyhow::Result;
use core::ops::Sub;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::configdb::{ConfigDb, EnabledWithValue};
use super::info::Info;
use crate::config::Config;
use crate::datetime::DateTime;

const DEFAULT_RANGE: usize = 7;

/// How often to run the retention job.  Currently 60 seconds.
const INTERVAL: u64 = 60;

/// The time to sleep between retention runs if not all old events
/// were deleted.
const REPEAT_INTERVAL: u64 = 1;

/// Number of events to delete per run.
const LIMIT: usize = 1000;

async fn get_size(configdb: &ConfigDb, config: &Config) -> Result<usize> {
    if let Ok(Some(size)) = config.get::<String>("database.retention.size") {
        if let Ok(size) = size.parse::<usize>() {
            return Ok(size);
        }
        if let Ok(size) = crate::util::parse_humansize(&size) {
            return Ok(size);
        }
        warn!("Invalid database.retention.size: {}", size);
    }

    let retention_size_config: Result<Option<EnabledWithValue>> =
        configdb.kv_get_config_as_t("config.retention.size").await;
    if let Ok(Some(config)) = retention_size_config {
        if config.enabled {
            return Ok(config.value as usize * 1000000000);
        }
    }

    Ok(0)
}

async fn get_days(configdb: &ConfigDb, config: &Config) -> Result<Option<usize>> {
    let days = if let Some(days) = config.get::<usize>("database.retention.days")? {
        debug!(
            "Found database.retention.days in configuration file of {} days",
            days
        );
        days
    } else if let Some(days) = config.get::<usize>("database.retention-period")? {
        debug!(
            "Found database.retention-period in configuration file of {} days",
            days
        );
        days
    } else {
        let retention_config: Result<Option<EnabledWithValue>> =
            configdb.kv_get_config_as_t("config.retention").await;
        match retention_config {
            Ok(None) => {
                // Not set in database, use default.
            }
            Ok(Some(config)) => {
                if config.enabled {
                    return Ok(Some(config.value as usize));
                }
            }
            Err(err) => {
                error!(
                    "Failed to get retention configuration will use default: error={}",
                    err
                );
            }
        }
        debug!(
            "Using default database retention period of {} days",
            DEFAULT_RANGE
        );
        DEFAULT_RANGE
    };
    if days > 0 {
        Ok(Some(days))
    } else {
        Ok(None)
    }
}

pub(crate) async fn start_retention_task(
    metrics: Arc<Metrics>,
    configdb: ConfigDb,
    config: Config,
    conn: Arc<tokio::sync::Mutex<SqliteConnection>>,
    filename: PathBuf,
) -> anyhow::Result<()> {
    tokio::spawn(async move {
        retention_task(metrics, config, configdb, conn, filename).await;
    });
    Ok(())
}

async fn size_enabled(conn: Arc<tokio::sync::Mutex<SqliteConnection>>) -> bool {
    let mut conn = conn.lock().await;
    let mut tx = conn.begin().await.unwrap();
    match Info::new(&mut tx).get_auto_vacuum().await {
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

async fn retention_task(
    metrics: Arc<Metrics>,
    config: Config,
    configdb: ConfigDb,
    conn: Arc<tokio::sync::Mutex<SqliteConnection>>,
    filename: PathBuf,
) {
    let size_enabled = size_enabled(conn.clone()).await;
    let default_delay = Duration::from_secs(INTERVAL);
    let report_interval = Duration::from_secs(60);

    // Short delay on startup.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let mut last_report = Instant::now();
    let mut count: u64 = 0;

    loop {
        let mut delay = default_delay;

        if !size_enabled {
            debug!("Size based database retention not available.");
        }
        if size_enabled {
            match get_size(&configdb, &config).await {
                Ok(size) => {
                    dbg!(size);
                    if size > 0 {
                        match delete_to_size(conn.clone(), &filename, size).await {
                            Err(err) => {
                                error!("Failed to delete database to max size: {:?}", err);
                            }
                            Ok(n) => {
                                dbg!(n);
                                if n > 0 {
                                    debug!(
                                        "Deleted {n} events to reduce database size to {} bytes",
                                        size
                                    );
                                    count += n;
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    error!(
                        "Failed to get database retention by size setting: {:?}",
                        err
                    );
                }
            }
        }

        if let Ok(Some(days)) = get_days(&configdb, &config).await {
            if days > 0 {
                match delete_older_than(conn.clone(), days as u64, LIMIT as u64).await {
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
            debug!("Events purged in last {:?}: {}", report_interval, count);
            count = 0;
            last_report = Instant::now();
        }

        if let Err(err) = auto_archive(&metrics, &configdb, conn.clone()).await {
            warn!("Failed to auto-archive events: {:?}", err);
        }

        tokio::time::sleep(delay).await;
    }
}

async fn auto_archive(
    metrics: &Metrics,
    configdb: &ConfigDb,
    conn: Arc<tokio::sync::Mutex<SqliteConnection>>,
) -> Result<()> {
    let config: Option<EnabledWithValue> =
        configdb.kv_get_config_as_t("config.autoarchive").await?;
    if let Some(config) = config {
        if config.enabled {
            let now = DateTime::now();
            let then = now.sub(Duration::from_secs(86400 * config.value));
            let mut conn = conn.lock().await;
            let action = HistoryEntryBuilder::new_auto_archived().build();
            let sql = r#"
                UPDATE events
                SET archived = 1,
                  history = json_insert(history, '$[#]', json(?))
                WHERE
                  json_extract(source, '$.event_type') = 'alert'
                  AND timestamp < ?
                  AND archived = 0"#;
            let mut tx = conn.begin().await?;
            let n = sqlx::query(sql)
                .bind(action.to_json())
                .bind(then.to_nanos())
                .execute(&mut *tx)
                .await?
                .rows_affected();
            tx.commit().await?;
            metrics.incr_autoarchived_by_age(n);
            debug!("Auto-archived {} alerts", n);
        }
    }

    Ok(())
}

async fn delete_to_size(
    conn: Arc<tokio::sync::Mutex<SqliteConnection>>,
    filename: &Path,
    bytes: usize,
) -> Result<u64> {
    let mut deleted = 0;
    loop {
        let file_size = crate::file::file_size(filename)? as usize;
        if file_size < bytes {
            debug!(
                "File size {} less than retention size limit of {}",
                file_size, bytes
            );
            break;
        }

        // Pause on subsequent rounds to let others get access to the
        // write lock.
        if deleted > 0 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        deleted += delete_oldest_events_n(conn.clone(), 1000).await?;
    }

    Ok(deleted)
}

async fn delete_older_than(
    conn: Arc<tokio::sync::Mutex<SqliteConnection>>,
    days: u64,
    limit: u64,
) -> Result<u64, sqlx::Error> {
    let mut conn = conn.lock().await;
    let now = DateTime::now();
    let period = std::time::Duration::from_secs(days * 86400);
    let older_than = now.sub(period);
    let timer = Instant::now();
    trace!("Deleting events older than {days} days");
    let sql = r#"DELETE FROM events
        WHERE rowid IN
            (SELECT rowid FROM
             events WHERE timestamp < ? 
               AND escalated = 0 
             ORDER BY timestamp ASC
             LIMIT ?)"#;
    let n = sqlx::query(sql)
        .bind(older_than.to_nanos())
        .bind(limit as i64)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    if n > 0 {
        debug!(
            "Deleted {n} events older than {} ({days} days) in {} ms",
            &older_than,
            timer.elapsed().as_millis()
        );
    } else {
        trace!("No events older than {days} days deleted");
    }
    Ok(n)
}

/// Delete events by oldest.
async fn delete_oldest_events_n(
    conn: Arc<tokio::sync::Mutex<SqliteConnection>>,
    limit: usize,
) -> Result<u64> {
    let sql = r#"DELETE FROM events
        WHERE rowid IN
            (SELECT rowid 
             FROM events
             WHERE escalated = 0
             ORDER BY timestamp ASC
             LIMIT ?)"#;
    let timer = Instant::now();
    let mut conn = conn.lock().await;
    let lock_elapsed = timer.elapsed();
    let n = sqlx::query(sql)
        .bind(limit as i64)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    let elapsed = timer.elapsed();
    let msg = format!(
        "Deleted {n} events in {:?} (lock-elapsed={:?})",
        elapsed, lock_elapsed
    );
    if elapsed > std::time::Duration::from_secs(1) {
        warn!("{}: Delete took longer than 1s", msg);
    } else {
        trace!("{}", msg);
    }

    Ok(n)
}
