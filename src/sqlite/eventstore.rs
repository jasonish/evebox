// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::prelude::*;
use std::sync::{Arc, Mutex};

use crate::eve::eve::EveJson;
use rusqlite::{params, Connection, Error, ToSql};
use serde_json::json;

use crate::datastore::DatastoreError;
use crate::elastic::AlertQueryOptions;
use crate::eve;
use crate::server::api::AlertGroupSpec;
use crate::sqlite::ConnectionBuilder;

impl From<rusqlite::Error> for DatastoreError {
    fn from(err: Error) -> Self {
        DatastoreError::SQLiteError(err)
    }
}

impl From<serde_json::Error> for DatastoreError {
    fn from(_: serde_json::Error) -> Self {
        DatastoreError::EventParseError
    }
}

/// SQLite implementation of the event datastore.
pub struct SQLiteEventStore {
    pub connection: Arc<Mutex<Connection>>,
    pub importer: super::importer::Importer,
    pub connection_builder: Arc<ConnectionBuilder>,
}

type QueryParam = dyn ToSql + Send + Sync + 'static;

impl SQLiteEventStore {
    pub fn new(connection_builder: Arc<ConnectionBuilder>) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection_builder.open().unwrap())),
            importer: super::importer::Importer::new(Arc::new(Mutex::new(
                connection_builder.open().unwrap(),
            ))),
            connection_builder,
        }
    }

    pub fn get_importer(&self) -> super::importer::Importer {
        self.importer.clone()
    }

    pub async fn alert_query(
        &self,
        options: AlertQueryOptions,
    ) -> Result<serde_json::Value, DatastoreError> {
        let mut conn = self.connection_builder.open().unwrap();

        let query = r#"
            SELECT b.count,
                a.rowid as id,
                b.mints as mints,
                b.escalated_count,
                a.archived,
                a.source
            FROM events a
                INNER JOIN
                (
                    SELECT
                        events.rowid,
                        count(json_extract(events.source, '$.alert.signature_id')) as count,
                        min(timestamp) as mints,
                        max(timestamp) as maxts,
                        sum(escalated) as escalated_count
                    FROM %FROM%
                    WHERE %WHERE%
                    GROUP BY
                        json_extract(events.source, '$.alert.signature_id'),
                        json_extract(events.source, '$.src_ip'),
                        json_extract(events.source, '$.dest_ip')
                ) AS b
            WHERE a.rowid = b.rowid AND
                a.timestamp = b.maxts
            ORDER BY timestamp DESC
        "#;

        let mut from: Vec<&str> = Vec::new();
        let mut filters: Vec<String> = Vec::new();
        let mut params: Vec<Box<QueryParam>> = Vec::new();

        from.push("events");

        filters.push("json_extract(events.source, '$.event_type') = ?".to_string());
        params.push(Box::new("alert"));

        for tag in options.tags {
            if tag == "archived" {
                filters.push("archived = ?".into());
                params.push(Box::new(1));
            } else if tag == "-archived" {
                filters.push("archived = ?".into());
                params.push(Box::new(0));
            } else if tag == "escalated" {
                filters.push("escalated = ?".into());
                params.push(Box::new(1));
            }
        }

        if let Some(ts) = options.timestamp_gte {
            filters.push("timestamp >= ?".into());
            params.push(Box::new(ts.timestamp_nanos()));
        }

        if let Some(query_string) = options.query_string {
            let mut query_string = query_string.as_str();
            let mut counter = 0;
            loop {
                counter += 1;
                if counter > 128 {
                    error!(
                        "Aborting query string parsing, too many iterations: {}",
                        query_string
                    );
                    break;
                }
                let (key, val, rem) = crate::sqlite::queryparser::parse_query_string(query_string);
                if let Some(key) = key {
                    if let Ok(val) = val.parse::<i64>() {
                        filters.push(format!("json_extract(events.source, '$.{}') = ?", key));
                        params.push(Box::new(val));
                    } else {
                        filters.push(format!("json_extract(events.source, '$.{}') LIKE ?", key));
                        params.push(Box::new(format!("%{}%", val)));
                    }
                } else if !val.is_empty() {
                    filters.push("events.source LIKE ?".into());
                    params.push(Box::new(format!("%{}%", val)));
                } else {
                    break;
                }
                query_string = rem;
            }
        }

        let query = query.replace("%WHERE%", &filters.join(" AND "));
        let query = query.replace("%FROM%", &from.join(", "));

        let map = |row: &rusqlite::Row| -> Result<serde_json::Value, rusqlite::Error> {
            let count: i64 = row.get(0)?;
            let id: i64 = row.get(1)?;
            let min_ts_nanos: i64 = row.get(2)?;

            let escalated_count: i64 = row.get(3)?;
            let archived: i8 = row.get(4)?;
            let mut parsed: serde_json::Value = row.get(5)?;

            if let serde_json::Value::Null = &parsed["tags"] {
                let tags: Vec<String> = Vec::new();
                parsed["tags"] = tags.into();
            }

            if let serde_json::Value::Array(ref mut tags) = &mut parsed["tags"] {
                if archived > 0 {
                    tags.push("archived".into());
                    tags.push("evebox.archived".into());
                }
            }

            use chrono::offset::TimeZone;
            let min_ts = chrono::Utc.timestamp_nanos(min_ts_nanos);

            let alert = json!({
                "count": count,
                "event": {
                    "_id": id,
                    "_source": parsed,
                },
                "minTs": super::format_sqlite_timestamp(&min_ts),
                "maxTs": &parsed["timestamp"],
                "escalatedCount": escalated_count,
            });

            Ok(alert)
        };

        let alerts = self
            .retry_query_loop(&mut conn, &query, &params, map)
            .await
            .unwrap();
        let response = json!({
            "alerts": alerts,
        });
        return Ok(response);
    }

    /// Run a database query in a loop as lock errors can occur, and we should retry those.
    async fn retry_query_loop<'a, F, T>(
        &'a self,
        conn: &'a mut Connection,
        query: &'a str,
        params: &Vec<Box<QueryParam>>,
        f: F,
    ) -> anyhow::Result<Vec<T>>
    where
        F: FnMut(&rusqlite::Row<'_>) -> Result<T, rusqlite::Error> + Copy,
    {
        let mut trys = 0;
        loop {
            match self.query_and_then(conn, query, params, f) {
                Ok(result) => {
                    return Ok(result);
                }
                Err(err) => {
                    if trys < 100 && err.to_string().contains("lock") {
                        trys += 1;
                    } else {
                        return Err(err.into());
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// Wrapper around Rusqlite's query_and_then but encapsulates its all to make it easier
    /// to run from a retry loop.
    fn query_and_then<F, T>(
        &self,
        conn: &mut Connection,
        query: &str,
        params: &Vec<Box<QueryParam>>,
        f: F,
    ) -> anyhow::Result<Vec<T>>
    where
        F: FnMut(&rusqlite::Row<'_>) -> Result<T, rusqlite::Error>,
    {
        let tx = conn.transaction()?;
        let mut stmt = tx.prepare(&query)?;
        let rows = stmt.query_and_then(rusqlite::params_from_iter(params), f)?;
        let mut out_rows = Vec::new();
        for row in rows {
            out_rows.push(row?);
        }
        return Ok(out_rows);
    }

    pub async fn event_query(
        &self,
        options: crate::datastore::EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        let mut conn = self.connection_builder.open()?;

        let query = r#"
            SELECT 
                events.rowid AS id, 
                events.archived AS archived, 
                events.escalated AS escalated, 
                events.source AS source
            FROM %FROM%
            WHERE %WHERE%
            ORDER BY events.timestamp %ORDER%
            LIMIT 500
        "#;

        let mut from: Vec<&str> = Vec::new();
        let mut filters: Vec<String> = Vec::new();
        let mut params: Vec<Box<QueryParam>> = Vec::new();

        from.push("events");
        filters.push("json_extract(events.source, '$.event_type') != 'stats'".to_string());

        if let Some(event_type) = options.event_type {
            filters.push("json_extract(events.source, '$.event_type') = ?".to_string());
            params.push(Box::new(event_type));
        }

        if let Some(dt) = options.max_timestamp {
            filters.push("timestamp <= ?".to_string());
            params.push(Box::new(dt.timestamp_nanos()));
        }

        if let Some(dt) = options.min_timestamp {
            filters.push("timestamp >= ?".to_string());
            params.push(Box::new(dt.timestamp_nanos()));
        }

        // Query string.
        if let Some(query_string) = options.query_string {
            let mut query_string = query_string.as_str();
            let mut counter = 0;
            loop {
                if query_string.is_empty() {
                    // Nothing left to parse.
                    break;
                }

                // Escape hatch in case of an infinite loop bug in the query parser as it could
                // use more testing.
                if counter > 100 {
                    error!(
                        "Aborting query string parsing, too many iterations: {}",
                        query_string
                    );
                    break;
                }

                let (key, val, rem) = crate::sqlite::queryparser::parse_query_string(query_string);
                if let Some(key) = key {
                    if let Ok(val) = val.parse::<i64>() {
                        filters.push(format!("json_extract(events.source, '$.{}') = ?", key));
                        params.push(Box::new(val));
                    } else {
                        filters.push(format!("json_extract(events.source, '$.{}') LIKE ?", key));
                        params.push(Box::new(format!("%{}%", val)));
                    }
                } else if !val.is_empty() {
                    filters.push("events.source LIKE ?".into());
                    params.push(Box::new(format!("%{}%", val)));
                }
                query_string = rem;
                counter += 1;
            }
        }

        let order = if let Some(order) = options.order {
            order
        } else {
            "DESC".to_string()
        };

        let query = query.replace("%FROM%", &from.join(", "));
        let query = query.replace("%WHERE%", &filters.join(" AND "));
        let query = query.replace("%ORDER%", &order);

        let mapper = |row: &rusqlite::Row| -> Result<serde_json::Value, rusqlite::Error> {
            let id: i64 = row.get(0)?;
            let archived: i8 = row.get(1)?;
            let escalated: i8 = row.get(2)?;
            let mut parsed: EveJson = row.get(3)?;

            if let Some(timestamp) = parsed.get("timestamp") {
                parsed["@timestamp"] = timestamp.clone();
            }

            if let serde_json::Value::Null = &parsed["tags"] {
                let tags: Vec<String> = Vec::new();
                parsed["tags"] = tags.into();
            }

            if let serde_json::Value::Array(ref mut tags) = &mut parsed["tags"] {
                if archived > 0 {
                    tags.push("archived".into());
                    tags.push("evebox.archived".into());
                }
                if escalated > 0 {
                    tags.push("escalated".into());
                    tags.push("evebox.escalated".into());
                }
            }

            let event = json!({
                "_id": id,
                "_source": parsed,
            });
            Ok(event)
        };

        let events = self
            .retry_query_loop(&mut conn, &query, &params, mapper)
            .await?;

        let response = json!({
            "data": events,
        });

        Ok(response)
    }

    pub async fn get_event_by_id(
        &self,
        event_id: String,
    ) -> Result<Option<serde_json::Value>, DatastoreError> {
        let conn = self.connection.lock().unwrap();
        let query = "SELECT rowid, archived, escalated, source FROM events WHERE rowid = ?";
        let params = params![event_id];
        let mut stmt = conn.prepare(query)?;
        let mut rows = stmt.query(params)?;
        if let Some(row) = rows.next()? {
            let rowid: i64 = row.get(0)?;
            let archived: i8 = row.get(1)?;
            let escalated: i8 = row.get(2)?;
            let mut parsed: EveJson = row.get(3)?;

            if let serde_json::Value::Null = &parsed["tags"] {
                let tags: Vec<String> = Vec::new();
                parsed["tags"] = tags.into();
            }

            if let serde_json::Value::Array(ref mut tags) = &mut parsed["tags"] {
                if archived > 0 {
                    tags.push("archived".into());
                    tags.push("evebox.archived".into());
                }
                if escalated > 0 {
                    tags.push("escalated".into());
                    tags.push("evebox.escalated".into());
                }
            }

            let response = json!({
                "_id": rowid,
                "_source": parsed,
            });
            return Ok(Some(response));
        }
        Ok(None)
    }

    // TODO: Unsure if the current query string needs to be considered. The Go code didn't
    //          consider it.
    pub async fn archive_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        debug!("Archiving alert group: {:?}", alert_group);
        let sql = "
            UPDATE events
            SET archived = 1
            WHERE %WHERE%
        ";

        let mut filters: Vec<String> = Vec::new();
        let mut params: Vec<Box<QueryParam>> = Vec::new();

        filters.push("json_extract(events.source, '$.event_type') = ?".to_string());
        params.push(Box::new("alert".to_string()));

        filters.push("archived = 0".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        params.push(Box::new(alert_group.signature_id as i64));

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        params.push(Box::new(alert_group.src_ip));

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        params.push(Box::new(alert_group.dest_ip));

        let mints = eve::parse_eve_timestamp(&alert_group.min_timestamp)?;
        let mints_nanos = mints.timestamp_nanos();
        filters.push("timestamp >= ?".to_string());
        params.push(Box::new(mints_nanos));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        let maxts_nanos = maxts.timestamp_nanos();
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts_nanos));

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        let mut conn = self.connection_builder.open()?;
        let n = self.retry_execute_loop(&mut conn, &sql, &params).await?;
        debug!("Archived {} alerts", n);
        Ok(())
    }

    async fn retry_execute_loop(
        &self,
        conn: &mut Connection,
        sql: &str,
        params: &Vec<Box<QueryParam>>,
    ) -> Result<usize, rusqlite::Error> {
        let start_time = std::time::Instant::now();
        loop {
            match conn.execute(&sql, rusqlite::params_from_iter(params)) {
                Ok(n) => {
                    return Ok(n);
                }
                Err(err) => {
                    if start_time.elapsed().as_millis() > 1000 {
                        return Err(err);
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        let sql = "
            UPDATE events
            SET escalated = 1
            WHERE %WHERE%
        ";

        let mut filters: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        filters.push("json_extract(events.source, '$.event_type') = ?".to_string());
        params.push(Box::new("alert".to_string()));

        filters.push("escalated = 0".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        params.push(Box::new(alert_group.signature_id as i64));

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        params.push(Box::new(alert_group.src_ip));

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        params.push(Box::new(alert_group.dest_ip));

        let mints = eve::parse_eve_timestamp(&alert_group.min_timestamp)?;
        filters.push("timestamp >= ?".to_string());
        params.push(Box::new(mints.timestamp_nanos()));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts.timestamp_nanos()));

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));
        let conn = self.connection.lock().unwrap();
        let n = conn.execute(&sql, rusqlite::params_from_iter(params))?;
        info!("Escalated {} alerts in alert group", n);
        Ok(())
    }

    pub async fn deescalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        let sql = "
            UPDATE events
            SET escalated = 0
            WHERE %WHERE%
        ";

        let mut filters: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        filters.push("json_extract(events.source, '$.event_type') = ?".to_string());
        params.push(Box::new("alert".to_string()));

        filters.push("escalated = 1".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        params.push(Box::new(alert_group.signature_id as i64));

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        params.push(Box::new(alert_group.src_ip));

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        params.push(Box::new(alert_group.dest_ip));

        let mints = eve::parse_eve_timestamp(&alert_group.min_timestamp)?;
        filters.push("timestamp >= ?".to_string());
        params.push(Box::new(mints.timestamp_nanos()));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts.timestamp_nanos()));

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));
        let conn = self.connection.lock().unwrap();
        let n = conn.execute(&sql, rusqlite::params_from_iter(params))?;
        info!("De-escalated {} alerts in alert group", n);
        Ok(())
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let conn = self.connection.lock().unwrap();
        let query = "UPDATE events SET archived = 1 WHERE rowid = ?";
        let params = params![event_id];
        let n = conn.execute(&query, params)?;
        if n == 0 {
            Err(DatastoreError::EventNotFound)
        } else {
            Ok(())
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let conn = self.connection.lock().unwrap();
        let query = "UPDATE events SET escalated = 1 WHERE rowid = ?";
        let params = params![event_id];
        let n = conn.execute(&query, params)?;
        if n == 0 {
            Err(DatastoreError::EventNotFound)
        } else {
            Ok(())
        }
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let conn = self.connection.lock().unwrap();
        let query = "UPDATE events SET escalated = 0 WHERE rowid = ?";
        let params = params![event_id];
        let n = conn.execute(&query, params)?;
        if n == 0 {
            Err(DatastoreError::EventNotFound)
        } else {
            Ok(())
        }
    }
}
