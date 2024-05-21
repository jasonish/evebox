// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::eventrepo::DatastoreError;
use crate::server::api::AlertGroupSpec;
use crate::sqlite::log_query_plan;
use crate::{LOG_QUERIES, LOG_QUERY_PLAN};
use serde_json::json;
use sqlx::sqlite::SqliteArguments;
use sqlx::Arguments;
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info};

mod alerts;
mod dhcp;
mod events;
mod groupby;
mod stats;

/// SQLite implementation of the event datastore.
pub(crate) struct SqliteEventRepo {
    pub importer: super::importer::SqliteEventSink,
    pub pool: SqlitePool,
    pub fts: bool,
}

impl SqliteEventRepo {
    pub fn new(
        conn: Arc<tokio::sync::Mutex<SqliteConnection>>,
        pool: SqlitePool,
        fts: bool,
    ) -> Self {
        debug!("SQLite event store created: fts={fts}");
        Self {
            importer: super::importer::SqliteEventSink::new(conn, fts),
            pool,
            fts,
        }
    }

    pub fn get_importer(&self) -> super::importer::SqliteEventSink {
        self.importer.clone()
    }

    pub async fn min_row_id(&self) -> Result<u64, DatastoreError> {
        let id = sqlx::query_scalar("SELECT MIN(rowid) FROM events")
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);
        Ok(id)
    }

    pub async fn max_row_id(&self) -> Result<u64, DatastoreError> {
        let id = sqlx::query_scalar("SELECT MAX(rowid) FROM events")
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);
        Ok(id)
    }

    pub async fn min_timestamp(&self) -> Result<Option<DateTime>, DatastoreError> {
        let result: Option<i64> = sqlx::query_scalar("SELECT MIN(timestamp) FROM events")
            .fetch_optional(&self.pool)
            .await?;
        if let Some(ts) = result {
            Ok(Some(crate::datetime::DateTime::from_nanos(ts)))
        } else {
            Ok(None)
        }
    }

    pub async fn max_timestamp(&self) -> Result<Option<DateTime>, DatastoreError> {
        let result: Option<i64> = sqlx::query_scalar("SELECT MAX(timestamp) FROM events")
            .fetch_optional(&self.pool)
            .await?;
        if let Some(ts) = result {
            Ok(Some(crate::datetime::DateTime::from_nanos(ts)))
        } else {
            Ok(None)
        }
    }

    pub async fn get_event_by_id(
        &self,
        event_id: String,
    ) -> Result<Option<serde_json::Value>, DatastoreError> {
        let sql = "SELECT rowid, archived, escalated, source FROM events WHERE rowid = ?";
        if let Some(row) = sqlx::query(sql)
            .bind(event_id)
            .fetch_optional(&self.pool)
            .await?
        {
            let rowid: i64 = row.try_get(0)?;
            let archived: i8 = row.try_get(1)?;
            let escalated: i8 = row.try_get(2)?;
            let mut parsed: serde_json::Value = row.try_get(3)?;

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

        let mut args = SqliteArguments::default();
        let mut filters: Vec<String> = Vec::new();

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());
        filters.push("archived = 0".to_string());

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        args.add(alert_group.signature_id as i64);

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        args.add(alert_group.src_ip);

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        args.add(&alert_group.dest_ip);

        let mints_nanos = crate::datetime::parse(&alert_group.min_timestamp, None)?.to_nanos();
        filters.push("timestamp >= ?".to_string());
        args.add(mints_nanos as i64);

        let maxts_nanos = crate::datetime::parse(&alert_group.max_timestamp, None)?.to_nanos();
        filters.push("timestamp <= ?".to_string());
        args.add(maxts_nanos as i64);

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        if *LOG_QUERIES {
            info!("sql={}", &sql);
        }

        let x = sqlx::query_with(&sql, args).execute(&self.pool).await?;
        let n = x.rows_affected();
        debug!("Archived {n} alerts in {} ms", now.elapsed().as_millis());

        Ok(())
    }

    pub async fn set_escalation_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
        escalate: bool,
    ) -> Result<u64, DatastoreError> {
        let mut filters: Vec<String> = Vec::new();
        let mut args = SqliteArguments::default();

        let sql = "
            UPDATE events
            SET escalated = ?
            WHERE %WHERE%
        ";
        args.add(if escalate { 1 } else { 0 });

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());
        filters.push("escalated = ?".to_string());
        args.add(if escalate { 0 } else { 1 });

        filters.push("json_extract(events.source, '$.alert.signature_id') = ?".to_string());
        args.add(alert_group.signature_id as i64);

        filters.push("json_extract(events.source, '$.src_ip') = ?".to_string());
        args.add(alert_group.src_ip);

        filters.push("json_extract(events.source, '$.dest_ip') = ?".to_string());
        args.add(alert_group.dest_ip);

        let mints = crate::datetime::parse(&alert_group.min_timestamp, None)?;
        filters.push("timestamp >= ?".to_string());
        args.add(mints.to_nanos() as i64);

        let maxts = crate::datetime::parse(&alert_group.max_timestamp, None)?;
        filters.push("timestamp <= ?".to_string());
        args.add(maxts.to_nanos() as i64);

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));
        let r = sqlx::query_with(&sql, args).execute(&self.pool).await?;
        let n = r.rows_affected();
        Ok(n)
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        let n = self
            .set_escalation_by_alert_group(alert_group, true)
            .await?;
        debug!("Escalated {n} alerts in group");
        Ok(())
    }

    pub async fn deescalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        let n = self
            .set_escalation_by_alert_group(alert_group, false)
            .await?;
        debug!("De-escalated {n} alerts in group");
        Ok(())
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let n = sqlx::query("UPDATE events SET archived = 1 WHERE rowid = ?")
            .bind(event_id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if n == 0 {
            Err(DatastoreError::EventNotFound)
        } else {
            Ok(())
        }
    }

    async fn set_escalation_for_id(
        &self,
        event_id: &str,
        escalate: bool,
    ) -> Result<(), DatastoreError> {
        let n = sqlx::query("UPDATE events SET escalated = ? WHERE rowid = ?")
            .bind(if escalate { 1 } else { 0 })
            .bind(event_id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if n == 0 {
            Err(DatastoreError::EventNotFound)
        } else {
            Ok(())
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        self.set_escalation_for_id(event_id, true).await
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        self.set_escalation_for_id(event_id, false).await
    }

    pub async fn get_sensors(&self) -> anyhow::Result<Vec<String>> {
        // Turns out not putting a timestamp limit on this is much
        // faster.
        let sql = r#"
            SELECT DISTINCT json_extract(events.source, '$.host')
            FROM events
            WHERE json_extract(events.source, '$.host') IS NOT NULL
            "#;
        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, "sensors", sql, SqliteArguments::default()).await;
        }

        let rows: Vec<String> = sqlx::query_scalar(sql).fetch_all(&self.pool).await?;
        Ok(rows)
    }
}
