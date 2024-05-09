// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::builder::SqliteValue;
use crate::eve::{self, Eve};
use rusqlite::TransactionBehavior;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use time::macros::format_description;
use tracing::{debug, error};

#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("timestamp parse error")]
    TimestampParseError,
    #[error("event has no timestamp field")]
    TimestampMissing,
    #[error("sqlite error: {0}")]
    SQLiteError(#[from] rusqlite::Error),
}

pub(crate) struct QueuedStatement {
    pub(crate) statement: String,
    pub(crate) params: Vec<SqliteValue>,
}

pub(crate) struct SqliteEventSink {
    conn: Arc<Mutex<rusqlite::Connection>>,
    queue: Vec<QueuedStatement>,
    fts: bool,
}

impl Clone for SqliteEventSink {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            queue: Vec::new(),
            fts: self.fts,
        }
    }
}

/// Prepare SQL statements for adding an event to the database.
pub(crate) fn prepare_sql(
    event: &mut serde_json::Value,
    fts: bool,
) -> Result<Vec<QueuedStatement>, IndexError> {
    let ts = event
        .timestamp()
        .ok_or_else(|| IndexError::TimestampMissing)?;
    reformat_timestamps(event);
    let source_values = extract_values(event);
    let mut archived = 0;

    if let Some(actions) = event["alert"]["metadata"]["evebox-action"].as_array() {
        for action in actions {
            if let serde_json::Value::String(action) = action {
                if action == "archive" {
                    archived = 1;
                    break;
                }
            }
        }
    }

    eve::eve::add_evebox_metadata(event, None);

    let mut statements = vec![];

    let statement =
        "INSERT INTO events (timestamp, archived, source, source_values) VALUES (?, ?, ?, ?)";
    let params = vec![
        SqliteValue::I64(ts.unix_timestamp_nanos() as i64),
        SqliteValue::I64(archived),
        SqliteValue::String(event.to_string()),
        SqliteValue::String(source_values.clone()),
    ];

    statements.push(QueuedStatement {
        statement: statement.to_string(),
        params,
    });

    if fts {
        let statement =
            "INSERT INTO fts (rowid, timestamp, source_values) VALUES (last_insert_rowid(), ?, ?)";
        let params = vec![
            SqliteValue::I64(ts.unix_timestamp_nanos() as i64),
            SqliteValue::String(source_values),
        ];
        statements.push(QueuedStatement {
            statement: statement.to_string(),
            params,
        });
    }

    Ok(statements)
}

impl SqliteEventSink {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>, fts: bool) -> Self {
        Self {
            conn,
            queue: Vec::new(),
            fts,
        }
    }

    pub async fn submit(&mut self, mut event: serde_json::Value) -> Result<bool, IndexError> {
        for statement in prepare_sql(&mut event, self.fts)? {
            self.queue.push(statement);
        }
        Ok(false)
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        loop {
            match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
                Err(err) => {
                    error!("Failed to start transaction, will try again: {}", err);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
                Ok(tx) => {
                    let start = Instant::now();
                    let n = self.queue.len();
                    for r in &self.queue {
                        // Run the execute in a loop as we can get lock errors here as well.
                        //
                        // TODO: Break out to own function, but need to replace (or get rid of)
                        //    the mutex with an async aware mutex.
                        loop {
                            let mut st = tx.prepare_cached(&r.statement)?;
                            match st.execute(rusqlite::params_from_iter(&r.params)) {
                                Ok(_) => {
                                    break;
                                }
                                Err(err) => {
                                    error!("Insert statement failed: {err}");
                                    if err.to_string().contains("locked") {
                                        std::thread::sleep(std::time::Duration::from_millis(10));
                                    } else {
                                        return Err(anyhow!("execute: {:?}", err));
                                    }
                                }
                            }
                        }
                    }
                    if let Err(err) = tx.commit() {
                        let source = err.source();
                        error!(
                            "Failed to commit events: error={}, source={:?}",
                            err, source
                        );
                        return Err(IndexError::SQLiteError(err).into());
                    } else {
                        let count = if self.fts { n / 2 } else { n };
                        debug!(
                            "Committed {count} events in {} ms",
                            start.elapsed().as_millis()
                        );
                    }
                    self.queue.truncate(0);
                    return Ok(n);
                }
            }
        }
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
    let format = format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6][offset_hour sign:mandatory][offset_minute]"
    );
    if let Ok(dt) = eve::parse_eve_timestamp(ts) {
        dt.to_offset(time::UtcOffset::UTC).format(&format).unwrap()
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
