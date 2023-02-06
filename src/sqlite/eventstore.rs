// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::datastore::DatastoreError;
use crate::eve::eve::EveJson;
use crate::prelude::*;
use crate::searchquery::Element;
use crate::server::api::AlertGroupSpec;
use crate::sqlite::ConnectionBuilder;
use crate::{datastore, eve};
use rusqlite::{params, Connection, ToSql};
use serde_json::json;
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod alerts;
mod stats;

/// SQLite implementation of the event datastore.
pub struct SQLiteEventStore {
    pub connection: Arc<Mutex<Connection>>,
    pub importer: super::importer::Importer,
    pub connection_builder: Arc<ConnectionBuilder>,
    pub pool: deadpool_sqlite::Pool,
}

/// A type alias over ToSql allowing us to create vectors of parameters.
type QueryParam = dyn ToSql + Send + Sync + 'static;

#[derive(Default)]
struct ParamBuilder {
    pub params: Vec<Box<dyn ToSql + Send + Sync + 'static>>,
    pub debug: Vec<String>,
}

impl ParamBuilder {
    fn new() -> Self {
        Default::default()
    }

    fn push<T>(&mut self, v: T)
    where
        T: ToSql + Display + Send + Sync + 'static,
    {
        self.debug.push(v.to_string());
        self.params.push(Box::new(v));
    }
}

impl SQLiteEventStore {
    pub fn new(connection_builder: Arc<ConnectionBuilder>, pool: deadpool_sqlite::Pool) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection_builder.open().unwrap())),
            importer: super::importer::Importer::new(Arc::new(Mutex::new(
                connection_builder.open().unwrap(),
            ))),
            connection_builder,
            pool,
        }
    }

    pub fn get_importer(&self) -> super::importer::Importer {
        self.importer.clone()
    }

    pub async fn events(
        &self,
        options: datastore::EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        let result = self
            .pool
            .get()
            .await?
            .interact(
                move |conn| -> Result<Vec<serde_json::Value>, rusqlite::Error> {
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
                    let mut from: Vec<&str> = vec![];
                    let mut filters: Vec<String> = vec![];
                    let mut params = ParamBuilder::new();

                    from.push("events");

                    if let Some(event_type) = options.event_type {
                        filters.push("json_extract(events.source, '$.event_type') = ?".to_string());
                        params.push(event_type);
                    }

                    if let Some(dt) = options.max_timestamp {
                        filters.push("timestamp <= ?".to_string());
                        params.push(dt.unix_timestamp_nanos() as i64);
                    }

                    if let Some(dt) = options.min_timestamp {
                        filters.push("timestamp >= ?".to_string());
                        params.push(dt.unix_timestamp_nanos() as i64);
                    }
                    for element in &options.query_string_elements {
                        match element {
                            Element::String(val) => {
                                filters.push("events.source LIKE ?".into());
                                params.push(format!("%{val}%"));
                            }
                            Element::KeyVal(key, val) => {
                                if let Ok(val) = val.parse::<i64>() {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{key}') = ?"
                                    ));
                                    params.push(val);
                                } else {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{key}') LIKE ?"
                                    ));
                                    params.push(format!("%{val}%"));
                                }
                            }
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

                    // TODO: Cleanup query building.
                    let mut query = query.to_string();
                    if filters.is_empty() {
                        query = query.replace("WHERE", "");
                    }

                    let mapper =
                        |row: &rusqlite::Row| -> Result<serde_json::Value, rusqlite::Error> {
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

                    let tx = conn.transaction()?;
                    let mut st = tx.prepare(&query)?;
                    let rows =
                        st.query_and_then(rusqlite::params_from_iter(&params.params), mapper)?;
                    let mut events = vec![];
                    for row in rows {
                        events.push(row?);
                    }
                    Ok(events)
                },
            )
            .await??;
        let response = json!({
            "ecs": false,
            "events": result,
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
        let now = Instant::now();
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
        let mints_nanos = mints.unix_timestamp_nanos();
        filters.push("timestamp >= ?".to_string());
        params.push(Box::new(mints_nanos as i64));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        let maxts_nanos = maxts.unix_timestamp_nanos();
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts_nanos as i64));

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        let result = self
            .pool
            .get()
            .await?
            .interact(move |conn| Self::retry_execute_loop(conn, &sql, &params))
            .await?;

        match result {
            Ok(n) => {
                debug!("Archived {} alerts in {} ms", n, now.elapsed().as_millis());
            }
            Err(err) => {
                error!("Failed to archive alert group: error={:?}", err);
                return Err(err)?;
            }
        }

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
        params.push(Box::new(mints.unix_timestamp_nanos() as i64));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts.unix_timestamp_nanos() as i64));

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
        params.push(Box::new(mints.unix_timestamp_nanos() as i64));

        let maxts = eve::parse_eve_timestamp(&alert_group.max_timestamp)?;
        filters.push("timestamp <= ?".to_string());
        params.push(Box::new(maxts.unix_timestamp_nanos() as i64));

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

fn sqlite_format_interval(duration: time::Duration) -> i64 {
    duration.whole_seconds()
}

fn nanos_to_rfc3339(nanos: i128) -> anyhow::Result<String> {
    let ts = time::OffsetDateTime::from_unix_timestamp_nanos(nanos)?;
    let rfc3339 = ts.format(&time::format_description::well_known::Rfc3339)?;
    Ok(rfc3339)
}

fn parse_timestamp(
    timestamp: &str,
) -> Result<time::OffsetDateTime, Box<dyn std::error::Error + Sync + Send>> {
    // The webapp may send the timestamp with an improperly encoded +, which will be received
    // as space. Help the parsing out by replacing spaces with "+".
    let timestamp = timestamp.replace(' ', "+");
    let ts = percent_encoding::percent_decode_str(&timestamp).decode_utf8_lossy();
    Ok(eve::parse_eve_timestamp(&ts)?)
}
