// SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! PostgreSQL partition management for time-based event partitioning.
//!
//! Events are partitioned by day based on the timestamp field (TIMESTAMPTZ).
//! This allows for efficient retention management by dropping entire partitions rather
//! than deleting individual rows.

use crate::prelude::*;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::RwLock;

/// Manages partition creation and caching to avoid repeated database lookups.
pub(crate) struct PartitionManager {
    pool: PgPool,
    /// Cache of partition names that are known to exist.
    known_partitions: RwLock<HashSet<String>>,
}

impl PartitionManager {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            known_partitions: RwLock::new(HashSet::new()),
        }
    }

    /// Generate the partition name for a given timestamp.
    pub fn partition_name(ts: &DateTime<Utc>) -> String {
        format!("events_{}", ts.format("%Y_%m_%d"))
    }

    /// Ensure a partition exists for the given timestamp.
    /// Uses an in-memory cache to avoid repeated database calls.
    pub async fn ensure_partition(&self, ts: &DateTime<Utc>) -> Result<String> {
        let partition_name = Self::partition_name(ts);

        // Check cache first
        {
            let known = self.known_partitions.read().unwrap();
            if known.contains(&partition_name) {
                return Ok(partition_name);
            }
        }

        // Not in cache, call database function to ensure it exists
        let result: String = sqlx::query_scalar("SELECT evebox_ensure_partition($1)")
            .bind(ts)
            .fetch_one(&self.pool)
            .await?;

        // Add to cache
        {
            let mut known = self.known_partitions.write().unwrap();
            known.insert(result.clone());
        }

        Ok(result)
    }

    /// Ensure partitions exist for all timestamps in a batch.
    /// Returns the set of partition names that were ensured.
    pub async fn ensure_partitions_for_batch(
        &self,
        timestamps: &[DateTime<Utc>],
    ) -> Result<HashSet<String>> {
        let mut partitions_needed: HashSet<String> = HashSet::new();
        let mut partitions_to_create: Vec<DateTime<Utc>> = Vec::new();

        // Determine which partitions we need
        {
            let known = self.known_partitions.read().unwrap();
            for ts in timestamps {
                let partition_name = Self::partition_name(ts);
                if !known.contains(&partition_name) && !partitions_needed.contains(&partition_name)
                {
                    partitions_needed.insert(partition_name);
                    partitions_to_create.push(*ts);
                }
            }
        }

        // Create missing partitions
        for ts in partitions_to_create {
            let partition_name = self.ensure_partition(&ts).await?;
            partitions_needed.insert(partition_name);
        }

        Ok(partitions_needed)
    }

    /// Drop partitions older than the specified number of days.
    /// Returns the number of partitions dropped.
    pub async fn drop_old_partitions(&self, retention_days: i32) -> Result<i32> {
        let dropped: i32 = sqlx::query_scalar("SELECT evebox_drop_old_partitions($1)")
            .bind(retention_days)
            .fetch_one(&self.pool)
            .await?;

        // Clear the cache since partitions may have been dropped
        if dropped > 0 {
            let mut known = self.known_partitions.write().unwrap();
            known.clear();
        }

        Ok(dropped)
    }

    /// List all partitions with their metadata.
    pub async fn list_partitions(&self) -> Result<Vec<PartitionInfo>> {
        let rows = sqlx::query_as::<_, PartitionInfo>(
            "SELECT partition_name, start_date, end_date, row_count FROM evebox_list_partitions()",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

/// Information about a partition.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct PartitionInfo {
    pub partition_name: String,
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
    pub row_count: i64,
}
