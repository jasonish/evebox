// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use crate::{eve::Eve, sqlite::has_table, sqlite::EveBoxSqlxErrorExt};
use anyhow::Context;
use rusqlite::TransactionBehavior;
use sqlx::Connection;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct SqliteEventConsumerMetrics {
    total_duration: Duration,
    occurrences: u64,
    events: usize,
    min: Duration,
    max: Duration,
    lock_errors: u64,
}

impl SqliteEventConsumerMetrics {
    fn update(&mut self, duration: Duration, events: usize) {
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

impl std::fmt::Display for SqliteEventConsumerMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let avg_duration = self.total_duration.as_millis() / self.occurrences as u128;
        let avg_duration = Duration::from_millis(avg_duration as u64);
        write!(
            f,
            "avg={:?}, occurrences={}, events={}, min={:?}, max={:?}, lock_errors={}",
            avg_duration, self.occurrences, self.events, self.min, self.max, self.lock_errors
        )
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum IndexError {
    #[error("event has no timestamp field")]
    TimestampMissing,
}

struct PreparedEvent {
    ts: i64,
    archived: u8,
    source_values: String,
    event: String,
}

pub(crate) struct SqliteEventSink {
    conn: Arc<tokio::sync::Mutex<sqlx::SqliteConnection>>,
    queue: Vec<PreparedEvent>,
    metrics: Arc<Mutex<SqliteEventConsumerMetrics>>,
    writer: Option<Arc<Mutex<rusqlite::Connection>>>,
    use_rusqlite: bool,
    server_metrics: Arc<crate::server::metrics::Metrics>,
}

impl Clone for SqliteEventSink {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            queue: Vec::new(),
            metrics: self.metrics.clone(),
            writer: self.writer.clone(),
            use_rusqlite: self.use_rusqlite,
            server_metrics: self.server_metrics.clone(),
        }
    }
}

impl SqliteEventSink {
    pub fn new(
        conn: Arc<tokio::sync::Mutex<sqlx::SqliteConnection>>,
        writer: Option<Arc<Mutex<rusqlite::Connection>>>,
        server_metrics: Arc<crate::server::metrics::Metrics>,
    ) -> Self {
        let use_rusqlite = match std::env::var("USE_RUSQLITE") {
            Ok(val) => matches!(val.as_ref(), "yes" | "1" | "true"),
            Err(_) => false,
        };
        if use_rusqlite && writer.is_some() {
            info!("SqliteEventSink will use Rusqlite");
        }
        Self {
            conn,
            queue: Vec::new(),
            metrics: server_metrics.sqlite_event_consumer.clone(),
            writer,
            use_rusqlite,
            server_metrics,
        }
    }

    fn prep(&mut self, mut event: serde_json::Value) -> Result<PreparedEvent, IndexError> {
        let ts = event.datetime().ok_or(IndexError::TimestampMissing)?;
        reformat_timestamps(&mut event);
        let source_values = extract_values(&event);
        let archived = if event.has_tag("evebox.archived") {
            1
        } else {
            0
        };
        let prepared = PreparedEvent {
            ts: ts.to_nanos(),
            source_values,
            event: event.to_string(),
            archived,
        };
        Ok(prepared)
    }

    pub async fn submit(&mut self, event: serde_json::Value) -> Result<bool, IndexError> {
        let prepared = self.prep(event)?;
        self.queue.push(prepared);
        Ok(false)
    }

    pub async fn commit_with_rusqlite(&mut self) -> anyhow::Result<usize> {
        let conn = self.writer.clone().unwrap().clone();
        let metrics = self.metrics.clone();
        let queue = std::mem::take(&mut self.queue);
        let n: usize = tokio::spawn(async move {
            let mut conn = conn.lock().unwrap();
            let timer = std::time::Instant::now();
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

            {
                let fts = tx
                    .query_row(
                        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'fts'",
                        [],
                        |row| {
                            let count: i32 = row.get(0).unwrap_or(0);
                            Ok(count > 0)
                        },
                    )
                    .unwrap_or(false);

                let mut events_st = tx.prepare_cached(
                    r#"
                            INSERT INTO events (timestamp, archived, source, source_values)
                            VALUES (?, ?, ?, ?)"#,
                )?;

                let mut fts_st = tx.prepare(
                    r#"
                            INSERT INTO fts (rowid, timestamp, source_values)
                            VALUES (last_insert_rowid(), ?, ?)"#,
                )?;

                for event in queue.iter() {
                    events_st.execute((
                        event.ts,
                        event.archived,
                        &event.event,
                        &event.source_values,
                    ))?;

                    if fts {
                        fts_st.execute((event.ts, &event.source_values))?;
                    }
                }
            }
            match tx.commit() {
                Ok(_) => {}
                Err(err) => {
                    error!("Commit failed: {:?}", err);
                }
            }

            let n = queue.len();

            let elapsed = timer.elapsed();
            let mut metrics = metrics.lock().unwrap();
            metrics.update(elapsed, n);

            if elapsed > Duration::from_secs(3) {
                warn!(
                    "Commit too longer than 3s: time={:?}, events={} -- {}",
                    elapsed,
                    n,
                    metrics.to_string()
                );
            } else {
                trace!(
                    "Committed {} events in time={:?} -- {}",
                    n,
                    elapsed,
                    metrics.to_string()
                );
            }

            Ok::<usize, anyhow::Error>(n)
        })
        .await??;
        Ok(n)
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        if self.use_rusqlite {
            self.commit_with_rusqlite().await
        } else {
            let mut tries = 0;
            loop {
                tries += 1;
                match self.commit_with_sqlx().await {
                    Ok(n) => return Ok(n),
                    Err(err) => {
                        if let Some(sqlxerr) = err.downcast_ref::<sqlx::Error>() {
                            if sqlxerr.is_locked() && tries < 35 {
                                if let Ok(mut metrics) = self.metrics.lock() {
                                    metrics.lock_errors += 1;
                                }
                                tokio::time::sleep(Duration::from_millis(2000)).await;
                                continue;
                            }
                        }
                        return Err(err).with_context(|| format!("retries={tries}"));
                    }
                }
            }
        }
    }

    async fn commit_with_sqlx(&mut self) -> anyhow::Result<usize> {
        let start = std::time::Instant::now();
        let lock_start = std::time::Instant::now();
        let mut conn = self.conn.lock().await;
        let lock_elapsed = lock_start.elapsed();

        let insert_start = std::time::Instant::now();
        let mut tx = conn
            .begin()
            .await
            .with_context(|| "Failed to begin transaction")?;
        let fts = has_table(&mut *tx, "fts").await?;
        for (count, event) in self.queue.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO events (timestamp, archived, source, source_values)
                VALUES (?, ?, ?, ?)
            "#,
            )
            .bind(event.ts)
            .bind(event.archived)
            .bind(&event.event)
            .bind(&event.source_values)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Insert into events failed: event #{count}"))?;

            if fts {
                sqlx::query(
                    r#"
                        INSERT INTO fts (rowid, timestamp, source_values)
                        VALUES (last_insert_rowid(), ?, ?)"#,
                )
                .bind(event.ts)
                .bind(&event.source_values)
                .execute(&mut *tx)
                .await
                .with_context(|| format!("Insert into fts failed: event #{count}"))?;
            }
        }
        let insert_elapsed = insert_start.elapsed();

        let commit_start = std::time::Instant::now();
        tx.commit()
            .await
            .with_context(|| "Transaction commit failed")?;
        let commit_elapsed = commit_start.elapsed();

        let n = self.queue.len();

        let elapsed = start.elapsed();
        let in_lock = insert_start.elapsed();

        let msg = format!(
            "Commited {n} events in {elapsed:?}: lock={lock_elapsed:?}, insert={insert_elapsed:?}, commit={commit_elapsed:?}"
        );

        let mut metrics = self.metrics.lock().unwrap();
        metrics.update(insert_elapsed + commit_elapsed, n);

        // For slow insert, that is the amount of time inside the
        // lock, log a message.
        //
        // While we do care about total time, which means time waiting
        // on the lock, I'm currently looking for slow activity in the
        // lock.
        if in_lock > Duration::from_secs(3) {
            warn!(
                "Commit took longer than 3s: {} -- {}",
                msg,
                metrics.to_string()
            );
        } else {
            trace!("{}: {}", msg, metrics.to_string());
        }

        self.queue.truncate(0);
        Ok(n)
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

/// Extract the values from the JSON and return them as a string,
/// space separated.
///
/// Simple values like null and bools are not returned. Also known
/// non-printable values (like base64 data) is not included. This is
/// used as the input to the full text search engine.
pub(crate) fn extract_values(input: &serde_json::Value) -> String {
    fn inner(input: &serde_json::Value, output: &mut String) {
        match input {
            serde_json::Value::Null | serde_json::Value::Bool(_) => {
                // Intentionally empty.
            }
            serde_json::Value::Number(n) => {
                if !output.is_empty() {
                    output.push(' ');
                }
                output.push_str(&n.to_string());
            }
            serde_json::Value::String(s) => {
                if !output.is_empty() {
                    output.push(' ');
                }
                output.push_str(s);
            }
            serde_json::Value::Array(a) => {
                for e in a {
                    inner(e, output);
                }
            }
            serde_json::Value::Object(o) => {
                for (k, v) in o {
                    match k.as_ref() {
                        "packet" | "payload" | "rule" => {}
                        _ => {
                            inner(v, output);
                        }
                    }
                }
            }
        }
    }

    let mut flattened = String::new();
    inner(input, &mut flattened);
    flattened
}
