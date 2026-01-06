// SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! PostgreSQL retention management using partition dropping.
//!
//! Unlike row-based deletion, partition-based retention is very efficient
//! as dropping a partition is essentially just dropping a table, which is
//! an O(1) operation regardless of data size.

use super::partition::PartitionManager;
use crate::config::Config;
use crate::prelude::*;
use crate::server::metrics::Metrics;
use crate::sqlite::configdb::{ConfigDb, EnabledWithValue};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_RETENTION_DAYS: i32 = 7;

/// How often to run the retention job (in seconds).
const INTERVAL: u64 = 3600; // 1 hour

/// Get the configured retention period in days.
async fn get_retention_days(configdb: &ConfigDb, config: &Config) -> Result<Option<i32>> {
    // Check configuration file first
    if let Some(days) = config.get::<i32>("database.retention.days")? {
        debug!(
            "Found database.retention.days in configuration file: {} days",
            days
        );
        return Ok(Some(days));
    }

    if let Some(days) = config.get::<i32>("database.retention-period")? {
        debug!(
            "Found database.retention-period in configuration file: {} days",
            days
        );
        return Ok(Some(days));
    }

    // Check database configuration
    let retention_config: Result<Option<EnabledWithValue>> =
        configdb.kv_get_config_as_t("config.retention").await;
    match retention_config {
        Ok(Some(config)) if config.enabled => {
            return Ok(Some(config.value as i32));
        }
        Ok(_) => {}
        Err(err) => {
            error!(
                "Failed to get retention configuration, will use default: error={}",
                err
            );
        }
    }

    debug!(
        "Using default database retention period of {} days",
        DEFAULT_RETENTION_DAYS
    );
    Ok(Some(DEFAULT_RETENTION_DAYS))
}

/// Start the retention task for PostgreSQL.
// TODO: Use metrics to record retention task activity (partitions dropped, errors, etc.)
pub(crate) fn start(
    _metrics: Arc<Metrics>,
    configdb: ConfigDb,
    config: Config,
    partition_manager: Arc<PartitionManager>,
) {
    tokio::spawn(async move {
        retention_task(configdb, config, partition_manager).await;
    });
}

async fn retention_task(
    configdb: ConfigDb,
    config: Config,
    partition_manager: Arc<PartitionManager>,
) {
    // Short delay on startup to let things settle.
    tokio::time::sleep(Duration::from_secs(10)).await;

    loop {
        match get_retention_days(&configdb, &config).await {
            Ok(Some(days)) if days > 0 => {
                info!("Running PostgreSQL retention check for {} days", days);
                match partition_manager.drop_old_partitions(days).await {
                    Ok(dropped) => {
                        if dropped > 0 {
                            info!("Dropped {} partition(s) older than {} days", dropped, days);
                        } else {
                            debug!("No partitions older than {} days to drop", days);
                        }
                    }
                    Err(err) => {
                        error!("Failed to drop old partitions: {:?}", err);
                    }
                }
            }
            Ok(Some(days)) => {
                debug!("Retention disabled (days={})", days);
            }
            Ok(None) => {
                debug!("Retention not configured");
            }
            Err(err) => {
                error!("Failed to get retention configuration: {:?}", err);
            }
        }

        // Log partition info periodically
        match partition_manager.list_partitions().await {
            Ok(partitions) => {
                if !partitions.is_empty() {
                    debug!("Current partitions:");
                    for p in &partitions {
                        let start = p
                            .start_date
                            .map(|d| d.to_string())
                            .unwrap_or_else(|| "?".to_string());
                        let end = p
                            .end_date
                            .map(|d| d.to_string())
                            .unwrap_or_else(|| "?".to_string());
                        debug!(
                            "  {} ({} to {}): ~{} rows",
                            p.partition_name, start, end, p.row_count
                        );
                    }
                }
            }
            Err(err) => {
                warn!("Failed to list partitions: {:?}", err);
            }
        }

        tokio::time::sleep(Duration::from_secs(INTERVAL)).await;
    }
}
