// SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::partition::PartitionManager;
use crate::eve::Eve;
use crate::eve::extract_values;
use crate::prelude::*;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub(crate) enum IndexError {
    #[error("event has no timestamp field")]
    TimestampMissing,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct PostgresEventConsumerMetrics {
    total_duration: Duration,
    occurrences: u64,
    events: usize,
    min: Duration,
    max: Duration,
}

impl PostgresEventConsumerMetrics {
    pub(crate) fn update(&mut self, duration: Duration, events: usize) {
        if duration > self.max {
            self.max = duration;
        }
        if self.occurrences == 0 || duration < self.min {
            self.min = duration;
        }

        self.total_duration += duration;
        self.occurrences += 1;
        self.events += events;
    }
}

impl std::fmt::Display for PostgresEventConsumerMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.occurrences == 0 {
            return write!(f, "no events yet");
        }
        let avg_duration = self.total_duration.as_millis() / self.occurrences as u128;
        let avg_duration = Duration::from_millis(avg_duration as u64);
        let eps = if self.total_duration.as_secs() > 0 {
            self.events as f64 / self.total_duration.as_secs_f64()
        } else {
            0.0
        };
        write!(
            f,
            "avg={:?}, eps={:.1}, occurrences={}, events={}, min={:?}, max={:?}",
            avg_duration, eps, self.occurrences, self.events, self.min, self.max
        )
    }
}

#[derive(Clone)]
struct PreparedEvent {
    ts: DateTime<Utc>,
    archived: bool,
    source_values: String,
    event: serde_json::Value,
}

#[derive(Clone)]
pub(crate) struct PostgresEventSink {
    pool: PgPool,
    partition_manager: Arc<PartitionManager>,
    queue: Vec<PreparedEvent>,
    metrics: Arc<Mutex<PostgresEventConsumerMetrics>>,
}

impl PostgresEventSink {
    pub fn new(
        pool: PgPool,
        partition_manager: Arc<PartitionManager>,
        metrics: Arc<Mutex<PostgresEventConsumerMetrics>>,
    ) -> Self {
        Self {
            pool,
            partition_manager,
            queue: Vec::new(),
            metrics,
        }
    }

    fn prep(&self, mut event: serde_json::Value) -> Result<PreparedEvent, IndexError> {
        let ts = event.datetime().ok_or(IndexError::TimestampMissing)?;
        reformat_timestamps(&mut event);
        // Sanitize the event to remove null bytes which PostgreSQL JSONB cannot store
        sanitize_json(&mut event);
        let archived = event.has_tag("evebox.archived");
        let source_values = extract_values(&event);
        Ok(PreparedEvent {
            ts: ts.datetime.to_utc(),
            event,
            archived,
            source_values,
        })
    }

    pub async fn submit(&mut self, event: serde_json::Value) -> Result<bool, IndexError> {
        let prepared = self.prep(event)?;
        self.queue.push(prepared);
        Ok(false)
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        if self.queue.is_empty() {
            return Ok(0);
        }

        let start = std::time::Instant::now();

        // Ensure partitions exist for all timestamps in the batch
        let timestamps: Vec<DateTime<Utc>> = self.queue.iter().map(|e| e.ts).collect();
        self.partition_manager
            .ensure_partitions_for_batch(&timestamps)
            .await?;

        // First try batch insert in a transaction (fast path)
        let events = std::mem::take(&mut self.queue);
        match self.try_batch_insert(&events).await {
            Ok(n) => {
                let elapsed = start.elapsed();
                self.update_metrics(elapsed, n);
                return Ok(n);
            }
            Err(err) => {
                debug!(
                    "Batch insert failed, falling back to individual inserts: {}",
                    err
                );
            }
        }

        // Fallback: insert events one by one (without transaction so failures don't abort)
        let mut committed = 0;
        let mut failed = 0;

        for event in &events {
            let result = sqlx::query(
                r#"
                INSERT INTO events (timestamp, archived, source, source_values)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(event.ts)
            .bind(event.archived)
            .bind(&event.event)
            .bind(&event.source_values)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => {
                    committed += 1;
                }
                Err(err) => {
                    failed += 1;
                    error!(
                        "Failed to insert event: error={}, event={}",
                        err, event.event
                    );
                }
            }
        }

        let elapsed = start.elapsed();
        self.update_metrics(elapsed, committed);
        if failed > 0 {
            warn!(
                "Committed {} events in {:?} ({} failed)",
                committed, elapsed, failed
            );
        }

        Ok(committed)
    }

    fn update_metrics(&self, elapsed: Duration, n: usize) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.update(elapsed, n);

        if elapsed > Duration::from_secs(3) {
            warn!(
                "Commit took longer than 3s: time={:?}, events={} -- {}",
                elapsed, n, metrics
            );
        } else {
            trace!("Committed {} events in {:?} -- {}", n, elapsed, metrics);
        }
    }

    /// Try to insert all events in a single transaction.
    async fn try_batch_insert(&self, events: &[PreparedEvent]) -> anyhow::Result<usize> {
        let mut tx = self.pool.begin().await?;

        for event in events {
            sqlx::query(
                r#"
                INSERT INTO events (timestamp, archived, source, source_values)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(event.ts)
            .bind(event.archived)
            .bind(&event.event)
            .bind(&event.source_values)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(events.len())
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }
}

fn reformat_timestamps(eve: &mut serde_json::Value) {
    if let serde_json::Value::String(ts) = &eve["timestamp"] {
        eve["timestamp"] = reformat_timestamp(ts).into();
    }

    if let serde_json::Value::String(ts) = &eve["flow"]["start"] {
        eve["flow"]["start"] = reformat_timestamp(ts).into();
    }

    if let serde_json::Value::String(ts) = &eve["flow"]["end"] {
        eve["flow"]["end"] = reformat_timestamp(ts).into();
    }
}

fn reformat_timestamp(ts: &str) -> String {
    if let Ok(dt) = crate::datetime::parse(ts, None) {
        dt.to_rfc3339_utc()
    } else {
        ts.to_string()
    }
}

/// Sanitize JSON for PostgreSQL JSONB storage.
///
/// PostgreSQL JSONB cannot store null bytes (\u0000) as it uses C-style
/// null-terminated strings internally. This function recursively removes
/// null bytes from all string values in the JSON.
fn sanitize_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            if s.contains('\0') {
                *s = s.replace('\0', "");
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                sanitize_json(item);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, v) in obj {
                sanitize_json(v);
            }
        }
        _ => {}
    }
}
