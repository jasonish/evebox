// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    eve::{self, filters::AutoArchiveFilter, Eve},
    sqlite::has_table,
};
use sqlx::Connection;
use std::sync::Arc;
use tracing::{debug, error};

#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("timestamp parse error")]
    TimestampParseError,
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
}

impl Clone for SqliteEventSink {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            queue: Vec::new(),
        }
    }
}

impl SqliteEventSink {
    pub fn new(conn: Arc<tokio::sync::Mutex<sqlx::SqliteConnection>>) -> Self {
        Self {
            conn,
            queue: Vec::new(),
        }
    }

    fn prep(&mut self, mut event: serde_json::Value) -> Result<PreparedEvent, IndexError> {
        let ts = event.datetime().ok_or(IndexError::TimestampMissing)?;
        reformat_timestamps(&mut event);
        let source_values = extract_values(&event);
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

        eve::eve::add_evebox_metadata(&mut event, None);
        AutoArchiveFilter::new().run(&mut event);

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

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        debug!("Committing {} events with sqlx", self.queue.len());
        let mut conn = self.conn.lock().await;
        let mut tx = conn.begin().await.unwrap();

        for event in &self.queue {
            sqlx::query::<sqlx::Sqlite>(
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
            .await?;

            if has_table(&mut *tx, "fts").await? {
                sqlx::query::<sqlx::Sqlite>(
                    r#"
                        INSERT INTO fts (rowid, timestamp, source_values)
                        VALUES (last_insert_rowid(), ?, ?)"#,
                )
                .bind(event.ts)
                .bind(&event.source_values)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        let n = self.queue.len();
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
