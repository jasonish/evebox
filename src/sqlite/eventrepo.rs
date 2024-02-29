// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::eventrepo::DatastoreError;
use crate::server::api::AlertGroupSpec;
use crate::{eve, LOG_QUERIES};
use rusqlite::{params, Connection, ToSql};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info};

mod alerts;
mod dhcp;
mod events;
mod groupby;
mod stats;

/// SQLite implementation of the event datastore.
pub struct SqliteEventRepo {
    pub connection: Arc<Mutex<Connection>>,
    pub importer: super::importer::SqliteEventSink,
    pub pool: deadpool_sqlite::Pool,
    pub fts: bool,
}

/// A type alias over ToSql allowing us to create vectors of parameters.
type QueryParam = dyn ToSql + Send + Sync + 'static;

impl SqliteEventRepo {
    pub fn new(connection: Arc<Mutex<Connection>>, pool: deadpool_sqlite::Pool, fts: bool) -> Self {
        debug!("SQLite event store created: fts={fts}");
        Self {
            connection: connection.clone(),
            importer: super::importer::SqliteEventSink::new(connection, fts),
            pool,
            fts,
        }
    }

    pub fn get_importer(&self) -> super::importer::SqliteEventSink {
        self.importer.clone()
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
            let mut parsed: serde_json::Value = row.get(3)?;

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
        let now = Instant::now();
        let sql = "
            UPDATE events
            SET archived = 1
            WHERE %WHERE%
        ";

        let mut filters: Vec<String> = Vec::new();
        let mut params: Vec<Box<QueryParam>> = Vec::new();

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());

        filters.push("archived = 0".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        params.push(Box::new(alert_group.signature_id as i64));

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        params.push(Box::new(alert_group.src_ip));

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        params.push(Box::new(alert_group.dest_ip));

        let mints = eve::parse_eve_timestamp(&alert_group.min_timestamp)?;
        let mints_nanos = mints.unix_timestamp_nanos();
        filters.push("timestamp >= ?".to_string());
        params.push(Box::new(mints_nanos as i64));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        let maxts_nanos = maxts.unix_timestamp_nanos();
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts_nanos as i64));

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        if *LOG_QUERIES {
            info!("sql={}", &sql);
        }

        let conn = self.connection.clone();
        let n = tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().unwrap();
            Self::retry_execute_loop(&mut conn, &sql, &params)
        })
        .await
        .unwrap()?;
        debug!("Archived {n} alerts in {} ms", now.elapsed().as_millis());

        Ok(())
    }

    fn retry_execute_loop(
        conn: &mut Connection,
        sql: &str,
        params: &[Box<QueryParam>],
    ) -> Result<usize, rusqlite::Error> {
        let start_time = std::time::Instant::now();
        loop {
            match conn.execute(sql, rusqlite::params_from_iter(params)) {
                Ok(n) => return Ok(n),
                Err(err) => {
                    if start_time.elapsed().as_millis() > 1000 {
                        return Err(err);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
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
        let mut params: Vec<Box<QueryParam>> = Vec::new();

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());

        filters.push("escalated = 0".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        params.push(Box::new(alert_group.signature_id as i64));

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        params.push(Box::new(alert_group.src_ip));

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        params.push(Box::new(alert_group.dest_ip));

        let mints = eve::parse_eve_timestamp(&alert_group.min_timestamp)?;
        filters.push("timestamp >= ?".to_string());
        params.push(Box::new(mints.unix_timestamp_nanos() as i64));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts.unix_timestamp_nanos() as i64));

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        let connection = self.connection.clone();
        let n = tokio::task::spawn_blocking(move || {
            let conn = connection.lock().unwrap();
            conn.execute(&sql, rusqlite::params_from_iter(params))
        })
        .await
        .unwrap()?;
        debug!("Escalated {n} alerts in alert group");

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
        let mut params: Vec<Box<QueryParam>> = Vec::new();

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());

        filters.push("escalated = 1".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        params.push(Box::new(alert_group.signature_id as i64));

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        params.push(Box::new(alert_group.src_ip));

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        params.push(Box::new(alert_group.dest_ip));

        let mints = eve::parse_eve_timestamp(&alert_group.min_timestamp)?;
        filters.push("timestamp >= ?".to_string());
        params.push(Box::new(mints.unix_timestamp_nanos() as i64));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts.unix_timestamp_nanos() as i64));

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        let connection = self.connection.clone();
        let n = tokio::task::spawn_blocking(move || {
            let conn = connection.lock().unwrap();
            conn.execute(&sql, rusqlite::params_from_iter(params))
        })
        .await
        .unwrap()?;
        debug!("De-escalated {n} alerts in alert group");
        Ok(())
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let conn = self.connection.lock().unwrap();
        let query = "UPDATE events SET archived = 1 WHERE rowid = ?";
        let params = params![event_id];
        let n = conn.execute(query, params)?;
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
        let n = conn.execute(query, params)?;
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
        let n = conn.execute(query, params)?;
        if n == 0 {
            Err(DatastoreError::EventNotFound)
        } else {
            Ok(())
        }
    }

    pub async fn get_sensors(&self) -> anyhow::Result<Vec<String>> {
        let start_time = time::OffsetDateTime::now_utc() - time::Duration::hours(24);
        let start_time = start_time.unix_timestamp_nanos() as i64;
        let result = self
            .pool
            .get()
            .await?
            .interact(move |conn| -> Result<Vec<String>, rusqlite::Error> {
                let sql = r#"
                    SELECT DISTINCT json_extract(events.source, '$.host')
                    FROM events
                    WHERE timestamp >= ?
                "#;
                let mut st = conn.prepare(sql).unwrap();
                let rows = st.query_map([&start_time], |row| row.get(0))?;
                let mut values = vec![];
                for row in rows {
                    values.push(row?);
                }
                Ok(values)
            })
            .await
            .map_err(|err| anyhow::anyhow!("sqlite interact error:: {:?}", err))??;
        Ok(result)
    }
}

fn nanos_to_rfc3339(nanos: i128) -> anyhow::Result<String> {
    let ts = time::OffsetDateTime::from_unix_timestamp_nanos(nanos)?;
    let rfc3339 = ts.format(&time::format_description::well_known::Rfc3339)?;
    Ok(rfc3339)
}
