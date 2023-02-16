// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::eve::eve::EveJson;
use crate::eve::{self, Eve};
use crate::prelude::*;
use rusqlite::types::ToSqlOutput;
use rusqlite::{ToSql, TransactionBehavior};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use time::macros::format_description;

#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("timestamp parse error")]
    TimestampParseError,
    #[error("timestamp missing")]
    TimestampMissing,
    #[error("sqlite error: {0}")]
    SQLiteError(#[from] rusqlite::Error),
}

enum Value {
    String(String),
    I64(i64),
}

impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self {
            Value::I64(v) => Ok(ToSqlOutput::Owned((*v).into())),
            Value::String(v) => Ok(ToSqlOutput::Owned(v.to_string().into())),
        }
    }
}

struct QueuedRecord {
    sql: String,
    params: Vec<Value>,
}

pub struct Importer {
    conn: Arc<Mutex<rusqlite::Connection>>,
    queue: Vec<QueuedRecord>,
    fts: bool,
}

impl Clone for Importer {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            queue: Vec::new(),
            fts: self.fts,
        }
    }
}

impl Importer {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>, fts: bool) -> Self {
        Self {
            conn,
            queue: Vec::new(),
            fts,
        }
    }

    pub async fn submit(&mut self, mut event: EveJson) -> Result<bool, IndexError> {
        let ts = match event.timestamp() {
            Some(ts) => ts,
            None => {
                return Err(IndexError::TimestampParseError);
            }
        };

        let mut source_values = String::new();
        reformat_timestamps(&mut event);
        flatten(&event, &mut source_values);

        eve::eve::add_evebox_metadata(&mut event, None);

        // Queue event insert.
        let sql = "INSERT INTO events (timestamp, source, source_values) VALUES (?1, ?2, ?3)";
        let params = vec![
            Value::I64(ts.unix_timestamp_nanos() as i64),
            Value::String(event.to_string()),
            Value::String(source_values.clone()),
        ];
        self.queue.push(QueuedRecord {
            sql: sql.to_string(),
            params,
        });

        if self.fts {
            let sql = "INSERT INTO fts (rowid, timestamp, source_values) VALUES (last_insert_rowid(), ?1, ?2)";
            let params = vec![
                Value::I64(ts.unix_timestamp_nanos() as i64),
                Value::String(source_values),
            ];
            self.queue.push(QueuedRecord {
                sql: sql.to_string(),
                params,
            });
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
                            let mut st = tx.prepare_cached(&r.sql)?;
                            match st.execute(rusqlite::params_from_iter(&r.params)) {
                                Ok(_) => {
                                    break;
                                }
                                Err(err) => {
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
                        debug!("Committed {n} events in {} ms", start.elapsed().as_millis());
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

fn reformat_timestamps(eve: &mut EveJson) {
    if let EveJson::String(ts) = &eve["timestamp"] {
        eve["timestamp"] = reformat_timestamp(ts).into();
    }

    if let EveJson::String(ts) = &eve["flow"]["start"] {
        eve["flow"]["start"] = reformat_timestamp(ts).into();
    }

    if let EveJson::String(ts) = &eve["flow"]["end"] {
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

pub(crate) fn flatten(input: &serde_json::Value, output: &mut String) {
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
                flatten(e, output);
            }
        }
        serde_json::Value::Object(o) => {
            for (k, v) in o {
                match k.as_ref() {
                    "packet" | "payload" | "rule" => {}
                    _ => {
                        flatten(v, output);
                    }
                }
            }
        }
    }
}
