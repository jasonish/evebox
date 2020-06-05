// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use crate::eve::eve::EveJson;
use crate::eve::{self, Eve};
use crate::logger::log;
use rusqlite::types::ToSqlOutput;
use rusqlite::ToSql;
use std::error::Error;
use std::sync::{Arc, Mutex};

#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("timestamp parse error")]
    TimestampParseError,
    #[error("timestamp missing")]
    TimestampMissing,
    #[error("sqlite error: {0}")]
    SQLiteError(rusqlite::Error),
}

impl From<rusqlite::Error> for IndexError {
    fn from(err: rusqlite::Error) -> Self {
        IndexError::SQLiteError(err)
    }
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

    pub async fn submit(&mut self, mut event: EveJson) -> Result<(), IndexError> {
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
        let mut params: Vec<Value> = Vec::new();
        params.push(Value::I64(ts.timestamp_nanos()));
        params.push(Value::String(event.to_string()));
        self.queue.push(QueuedRecord {
            sql: sql.to_string(),
            params,
        });

        // Queue FTS insert.
        let sql = "INSERT INTO events_fts (rowid, source) VALUES (last_insert_rowid(), ?1)";
        let mut params = Vec::new();
        params.push(Value::String(values.join(" ")));
        self.queue.push(QueuedRecord {
            sql: sql.to_string(),
            params,
        });

        Ok(())
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        log::debug!("Commiting {} events", self.pending());
        let mut conn = self.conn.lock().unwrap();
        loop {
            match conn.transaction() {
                Err(err) => {
                    log::error!("Failed to start transaction, will try again: {}", err);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
                Ok(tx) => {
                    let n = self.queue.len();
                    for r in &self.queue {
                        tx.execute(&r.sql, &r.params)?;
                    }
                    if let Err(err) = tx.commit() {
                        let source = err.source();
                        log::error!(
                            "Failed to commit events: error={}, source={:?}",
                            err,
                            source
                        );
                        return Err(IndexError::SQLiteError(err))?;
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
        eve["timestamp"] = reformat_timestamp(&ts).into();
    }

    if let EveJson::String(ts) = &eve["flow"]["start"] {
        eve["flow"]["start"] = reformat_timestamp(&ts).into();
    }

    if let EveJson::String(ts) = &eve["flow"]["end"] {
        eve["flow"]["end"] = reformat_timestamp(&ts).into();
    }
}

fn reformat_timestamp(ts: &str) -> String {
    if let Ok(dt) = eve::parse_eve_timestamp(ts) {
        super::format_sqlite_timestamp(&dt)
    } else {
        ts.to_string()
    }
}
