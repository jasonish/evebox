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
}

impl Clone for Importer {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            queue: Vec::new(),
        }
    }
}

impl Importer {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self {
            conn: conn,
            queue: Vec::new(),
        }
    }

    pub async fn submit(&mut self, mut event: EveJson) -> Result<bool, IndexError> {
        let ts = match event.timestamp() {
            Some(ts) => ts,
            None => {
                return Err(IndexError::TimestampParseError);
            }
        };

        let mut values = Vec::new();
        extract_values(&event, &mut values);
        reformat_timestamps(&mut event);

        eve::eve::add_evebox_metadata(&mut event, None);

        // Queue event insert.
        let sql = "INSERT INTO events (timestamp, source) VALUES (?1, ?2)";
        let params = vec![
            Value::I64(ts.unix_timestamp_nanos() as i64),
            Value::String(event.to_string()),
        ];
        self.queue.push(QueuedRecord {
            sql: sql.to_string(),
            params,
        });

        Ok(false)
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        debug!("Commiting {} events", self.pending());
        let mut conn = self.conn.lock().unwrap();
        loop {
            match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
                Err(err) => {
                    error!("Failed to start transaction, will try again: {}", err);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
                Ok(tx) => {
                    let n = self.queue.len();
                    for r in &self.queue {
                        // Run the execute in a loop as we can get lock errors here as well.
                        //
                        // TODO: Break out to own function, but need to replace (or get rid of)
                        //    the mutex with an async aware mutex.
                        loop {
                            match tx.execute(&r.sql, rusqlite::params_from_iter(&r.params)) {
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

fn extract_values(eve: &EveJson, values: &mut Vec<String>) {
    if eve.is_object() {
        if let serde_json::Value::Object(obj) = eve {
            for (_, v) in obj {
                match v {
                    serde_json::Value::String(s) => {
                        values.push(s.clone());
                    }
                    serde_json::Value::Number(n) => {
                        values.push(n.to_string());
                    }
                    serde_json::Value::Object(_) => {
                        extract_values(v, values);
                    }
                    serde_json::Value::Array(_) => {
                        extract_values(v, values);
                    }
                    _ => {}
                }
            }
        }
    } else if eve.is_array() {
        if let serde_json::Value::Array(a) = eve {
            for v in a {
                match v {
                    serde_json::Value::String(s) => {
                        values.push(s.clone());
                    }
                    serde_json::Value::Number(n) => {
                        values.push(n.to_string());
                    }
                    serde_json::Value::Object(_) => {
                        extract_values(v, values);
                    }
                    serde_json::Value::Array(_) => {
                        extract_values(v, values);
                    }
                    _ => {}
                }
            }
        }
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
