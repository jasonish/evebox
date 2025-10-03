// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use crate::sqlite::prelude::*;

use crate::datetime::DateTime;
use crate::elastic::HistoryEntryBuilder;
use crate::eve::eve::ensure_has_history;
use crate::server::api::AlertGroupSpec;
use crate::server::session::Session;
use crate::sqlite::log_query_plan;
use crate::{LOG_QUERIES, LOG_QUERY_PLAN};
use serde_json::json;
use sqlx::sqlite::SqliteArguments;
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

use super::has_table;

mod agg;
mod alerts;
mod comments;
mod dhcp;
mod dns;
mod events;
mod stats;

/// SQLite implementation of the event datastore.
pub(crate) struct SqliteEventRepo {
    pub importer: super::importer::SqliteEventSink,
    pub pool: SqlitePool,
    pub writer: Arc<tokio::sync::Mutex<SqliteConnection>>,
    pub _rusqlite_writer: Option<Arc<Mutex<rusqlite::Connection>>>,
}

impl SqliteEventRepo {
    pub fn new(
        writer: Arc<tokio::sync::Mutex<SqliteConnection>>,
        pool: SqlitePool,
        rusqlite_writer: Option<Arc<Mutex<rusqlite::Connection>>>,
        metrics: Arc<crate::server::metrics::Metrics>,
    ) -> Self {
        Self {
            importer: super::importer::SqliteEventSink::new(
                writer.clone(),
                rusqlite_writer.clone(),
                metrics,
            ),
            pool,
            writer: writer.clone(),
            _rusqlite_writer: rusqlite_writer,
        }
    }

    pub async fn fts(&self) -> bool {
        has_table(&self.pool, "fts").await.unwrap_or(false)
    }

    pub fn get_importer(&self) -> super::importer::SqliteEventSink {
        self.importer.clone()
    }

    pub async fn min_row_id(&self) -> Result<u64> {
        let sql = "SELECT MIN(rowid) FROM events";

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        let id = sqlx::query_scalar(sql)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);
        Ok(id)
    }

    pub async fn max_row_id(&self) -> Result<u64> {
        let sql = "SELECT MAX(rowid) FROM events";

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        let id = sqlx::query_scalar(sql)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);
        Ok(id)
    }

    /// Return the earliest/minimum timestamp found in the events
    /// table.
    pub(crate) async fn earliest_timestamp(&self) -> Result<Option<DateTime>> {
        let sql = "SELECT MIN(timestamp) FROM events";

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        let result: Option<i64> = sqlx::query_scalar(sql).fetch_optional(&self.pool).await?;
        if let Some(ts) = result {
            Ok(Some(crate::datetime::DateTime::from_nanos(ts)))
        } else {
            Ok(None)
        }
    }

    pub async fn max_timestamp(&self) -> Result<Option<DateTime>> {
        let sql = "SELECT MAX(timestamp) FROM events";

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        let result: Option<i64> = sqlx::query_scalar(sql).fetch_optional(&self.pool).await?;
        if let Some(ts) = result {
            Ok(Some(crate::datetime::DateTime::from_nanos(ts)))
        } else {
            Ok(None)
        }
    }

    pub async fn get_event_by_id(&self, event_id: String) -> Result<Option<serde_json::Value>> {
        let sql = r#"
            SELECT
              rowid, archived, escalated, source, history
            FROM events
            WHERE rowid = ?"#;

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        if let Some(row) = sqlx::query(sql)
            .bind(event_id)
            .fetch_optional(&self.pool)
            .await?
        {
            let rowid: i64 = row.try_get(0)?;
            let archived: i8 = row.try_get(1)?;
            let escalated: i8 = row.try_get(2)?;
            let mut parsed: serde_json::Value = row.try_get(3)?;
            let history: serde_json::Value = row.try_get("history")?;

            if let serde_json::Value::Null = &parsed["tags"] {
                let tags: Vec<String> = Vec::new();
                parsed["tags"] = tags.into();
            }

            if let serde_json::Value::Array(tags) = &mut parsed["tags"] {
                if archived > 0 && !tags.contains(&"evebox.archived".into()) {
                    tags.push("evebox.archived".into());
                }
                if escalated > 0 && !tags.contains(&"evebox.escalated".into()) {
                    tags.push("evebox.escalated".into());
                }
            }

            ensure_has_history(&mut parsed);
            parsed["evebox"]["history"] = history;

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
    pub async fn archive_by_alert_group(&self, alert_group: AlertGroupSpec) -> Result<u64> {
        debug!("Archiving alert group: {:?}", alert_group);

        let action = HistoryEntryBuilder::new_archived().build();
        let sql = "
            UPDATE events
            SET archived = 1,
              history = json_insert(history, '$[#]', json(?))
            WHERE %WHERE%
        ";

        let mut args = SqliteArguments::default();
        let mut filters: Vec<String> = Vec::new();

        args.push(action.to_json())?;

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());
        filters.push("archived = 0".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        args.push(alert_group.signature_id as i64)?;

        let src_ip = alert_group.src_ip.unwrap_or_default();
        if src_ip.is_empty() {
            filters.push("(json_extract(events.source, '$.src_ip') IS NULL OR json_extract(events.source, '$.src_ip') = '')".to_string());
        } else {
            filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
            args.push(src_ip)?;
        }

        let dest_ip = alert_group.dest_ip.unwrap_or_default();
        if dest_ip.is_empty() {
            filters.push("(json_extract(events.source, '$.dest_ip') IS NULL OR json_extract(events.source, '$.dest_ip') = '')".to_string());
        } else {
            filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
            args.push(dest_ip)?;
        }

        let mints_nanos = crate::datetime::parse(&alert_group.min_timestamp, None)?.to_nanos();
        filters.push("timestamp >= ?".to_string());
        args.push(mints_nanos as i64)?;

        let maxts_nanos = crate::datetime::parse(&alert_group.max_timestamp, None)?.to_nanos();
        filters.push("timestamp <= ?".to_string());
        args.push(maxts_nanos as i64)?;

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &args).await;
        }
        if *LOG_QUERIES {
            info!("sql={}", &sql);
        }

        let start = Instant::now();
        let mut conn = self.writer.lock().await;
        let write_lock_elapsed = start.elapsed();
        let n = sqlx::query_with(&sql, args)
            .execute(&mut *conn)
            .await?
            .rows_affected();
        let query_elapsed = start.elapsed();
        debug!(
            "Archived {n} alerts in {} ms (write-lock wait: {})",
            query_elapsed.as_millis(),
            write_lock_elapsed.as_millis()
        );

        Ok(n)
    }

    pub async fn set_escalation_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
        escalate: bool,
    ) -> Result<u64> {
        let mut filters: Vec<String> = Vec::new();
        let mut args = SqliteArguments::default();

        let action = if escalate {
            HistoryEntryBuilder::new_escalate()
        } else {
            HistoryEntryBuilder::new_deescalate()
        }
        .build();

        let sql = "
            UPDATE events
            SET escalated = ?,
              history = json_insert(history, '$[#]', json(?))
            WHERE %WHERE%
        ";
        args.push(if escalate { 1 } else { 0 })?;
        args.push(serde_json::to_string(&action).unwrap())?;

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());
        filters.push("escalated = ?".to_string());
        args.push(if escalate { 0 } else { 1 })?;

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        args.push(alert_group.signature_id as i64)?;

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        args.push(alert_group.src_ip)?;

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        args.push(alert_group.dest_ip)?;

        let mints = crate::datetime::parse(&alert_group.min_timestamp, None)?;
        filters.push("timestamp >= ?".to_string());
        args.push(mints.to_nanos() as i64)?;

        let maxts = crate::datetime::parse(&alert_group.max_timestamp, None)?;
        filters.push("timestamp <= ?".to_string());
        args.push(maxts.to_nanos() as i64)?;

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &args).await;
        }

        let mut conn = self.writer.lock().await;
        let start = Instant::now();
        let r = sqlx::query_with(&sql, args).execute(&mut *conn).await?;
        let n = r.rows_affected();
        info!(
            "Set {} events to escalated = {} in {:?}",
            n,
            escalate,
            start.elapsed()
        );
        Ok(n)
    }

    pub async fn escalate_by_alert_group(
        &self,
        _session: Arc<Session>,
        alert_group: AlertGroupSpec,
    ) -> Result<()> {
        let n = self
            .set_escalation_by_alert_group(alert_group, true)
            .await?;
        debug!("Escalated {n} alerts in group");
        Ok(())
    }

    pub async fn deescalate_by_alert_group(
        &self,
        _session: Arc<Session>,
        alert_group: AlertGroupSpec,
    ) -> Result<()> {
        let n = self
            .set_escalation_by_alert_group(alert_group, false)
            .await?;
        debug!("De-escalated {n} alerts in group");
        Ok(())
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<()> {
        let action = HistoryEntryBuilder::new_archived().build();
        let sql = r#"
            UPDATE events
            SET archived = 1,
              history = json_insert(history, '$[#]', json(?))
            WHERE rowid = ?"#;

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        let mut conn = self.writer.lock().await;
        let n = sqlx::query(sql)
            .bind(action.to_json())
            .bind(event_id)
            .execute(&mut *conn)
            .await?
            .rows_affected();
        if n == 0 {
            warn!("Archive by event ID request did not update any events");
            // TODO: Return true/false depending on if there was an event...
            bail!("sqlite: event not found");
        } else {
            Ok(())
        }
    }

    async fn set_escalation_for_id(&self, event_id: &str, escalate: bool) -> Result<()> {
        let action = if escalate {
            HistoryEntryBuilder::new_escalate()
        } else {
            HistoryEntryBuilder::new_deescalate()
        }
        .build();

        let sql = r#"
            UPDATE events 
            SET escalated = ?,
              history = json_insert(history, '$[#]', json(?))
            WHERE rowid = ?"#;

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        let mut conn = self.writer.lock().await;
        let n = sqlx::query(sql)
            .bind(if escalate { 1 } else { 0 })
            .bind(action.to_json())
            .bind(event_id)
            .execute(&mut *conn)
            .await?
            .rows_affected();
        if n == 0 {
            bail!("sqlite: event not found");
        } else {
            Ok(())
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<()> {
        self.set_escalation_for_id(event_id, true).await
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<()> {
        self.set_escalation_for_id(event_id, false).await
    }

    #[instrument(skip_all)]
    pub async fn get_sensors(&self) -> anyhow::Result<Vec<String>> {
        // Get sensors with host field
        // Single query to get all sensors, including NULL values
        let sql = r#"
            SELECT DISTINCT json_extract(events.source, '$.host')
            FROM events
            "#;
        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, sql, &SqliteArguments::default()).await;
        }

        let rows: Vec<Option<String>> = sqlx::query_scalar(sql).fetch_all(&self.pool).await?;

        // Map NULL values to "(no-name)" in Rust
        let mut sensors: Vec<String> = rows
            .into_iter()
            .map(|host| host.unwrap_or_else(|| "(no-name)".to_string()))
            .collect();

        // Sort sensors, keeping "(no-name)" at the end
        sensors.sort_by(|a, b| {
            if a == "(no-name)" {
                std::cmp::Ordering::Greater
            } else if b == "(no-name)" {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });

        Ok(sensors)
    }
}
