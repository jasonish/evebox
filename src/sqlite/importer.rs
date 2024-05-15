// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::eve::{self, Eve};
use sqlx::Connection;
use std::sync::Arc;
use time::macros::format_description;
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
    fts: bool,
}

impl Clone for SqliteEventSink {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            fts: self.fts,
            queue: Vec::new(),
        }
    }
}

impl SqliteEventSink {
    pub fn new(conn: Arc<tokio::sync::Mutex<sqlx::SqliteConnection>>, fts: bool) -> Self {
        Self {
            conn,
            fts,
            queue: Vec::new(),
        }
    }

    fn prep(&mut self, event: &mut serde_json::Value) -> Result<(), IndexError> {
        let ts = event.timestamp().ok_or(IndexError::TimestampMissing)?;
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

        let prepared = PreparedEvent {
            ts: ts.unix_timestamp_nanos() as i64,
            source_values,
            event: event.to_string(),
            archived,
        };

        self.queue.push(prepared);

        Ok(())
    }

    pub async fn submit(&mut self, mut event: serde_json::Value) -> Result<bool, IndexError> {
        self.prep(&mut event)?;
        Ok(false)
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        debug!("Committing {} events with sqlx", self.queue.len());
        let mut conn = self.conn.lock().await;
        let mut tx = conn.begin().await.unwrap();

        for event in &self.queue {
            let _result = sqlx::query::<sqlx::Sqlite>(
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

            if self.fts {
                let _result = sqlx::query::<sqlx::Sqlite>(
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
