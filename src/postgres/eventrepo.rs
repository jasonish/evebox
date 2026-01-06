// SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::partition::PartitionManager;
use crate::datetime::DateTime;
use crate::elastic::{AlertQueryOptions, HistoryEntryBuilder};
use crate::eve::eve::ensure_has_history;
use crate::eventrepo::{
    AggAlert, AggAlertMetadata, AlertsResult, EventQueryParams, StatsAggQueryParams,
};
use crate::postgres::importer::PostgresEventConsumerMetrics;
use crate::postgres::query_builder::EventQueryBuilder;
use crate::prelude::*;
use crate::queryparser;
use crate::server::api::AlertGroupSpec;
use crate::server::session::Session;
use futures::TryStreamExt;
use serde_json::json;
use sqlx::postgres::PgArguments;
use sqlx::{Arguments, PgPool, Row};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// PostgreSQL implementation of the event datastore.
pub(crate) struct PostgresEventRepo {
    pool: PgPool,
    partition_manager: Arc<PartitionManager>,
    importer: super::importer::PostgresEventSink,
}

impl PostgresEventRepo {
    pub fn new(pool: PgPool, metrics: Arc<Mutex<PostgresEventConsumerMetrics>>) -> Self {
        let partition_manager = Arc::new(PartitionManager::new(pool.clone()));
        let importer = super::importer::PostgresEventSink::new(
            pool.clone(),
            partition_manager.clone(),
            metrics,
        );
        Self {
            pool,
            partition_manager,
            importer,
        }
    }

    pub fn get_importer(&self) -> super::importer::PostgresEventSink {
        self.importer.clone()
    }

    /// Get a reference to the partition manager.
    pub fn partition_manager(&self) -> &Arc<PartitionManager> {
        &self.partition_manager
    }

    /// Return the earliest/minimum timestamp found in the events table.
    pub(crate) async fn earliest_timestamp(&self) -> Result<Option<DateTime>> {
        let result: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT MIN(timestamp) FROM events")
                .fetch_optional(&self.pool)
                .await?;
        Ok(result.map(DateTime::from))
    }

    /// Return a time-based histogram of event counts.
    pub(crate) async fn histogram_time(
        &self,
        interval: Option<u64>,
        query: &[queryparser::QueryElement],
    ) -> Result<Vec<serde_json::Value>> {
        use futures::TryStreamExt;
        use serde::Serialize;

        // The timestamp (in seconds) of the latest event to consider.
        // Used to determine bucket interval and fill holes at the end.
        let now = DateTime::now().to_seconds();

        let from = query
            .iter()
            .find(|e| matches!(e.value, queryparser::QueryValue::From(_)))
            .map(|e| match e.value {
                queryparser::QueryValue::From(ref v) => v,
                _ => unreachable!(),
            });

        let earliest = if let Some(from) = from {
            from.clone()
        } else if let Some(earliest) = self.earliest_timestamp().await? {
            earliest
        } else {
            return Ok(vec![]);
        };

        let interval = if let Some(interval) = interval {
            interval
        } else {
            let interval = crate::util::histogram_interval(now - earliest.to_seconds());
            debug!("No interval provided by client, using {interval}s");
            interval
        };

        let last_time = now / (interval as i64) * (interval as i64);
        let mut next_time = ((earliest.to_seconds() as u64) / interval * interval) as i64;

        let mut builder = EventQueryBuilder::new();
        builder.select(format!(
            "(EXTRACT(EPOCH FROM timestamp)::bigint / {interval}) * {interval} AS bucket_time"
        ));
        builder.select("COUNT(*) AS count");
        builder.from("events");
        builder.apply_query_string(query)?;

        // Build the query manually to add GROUP BY
        let (base_sql, args) = builder.build()?;

        // Insert GROUP BY before any ORDER BY or at the end
        let sql = format!("{} GROUP BY bucket_time ORDER BY bucket_time ASC", base_sql);

        debug!("Histogram time query: {}", &sql);

        #[derive(Debug, Serialize)]
        struct Element {
            time: i64,
            count: u64,
            debug: String,
        }

        let mut results = vec![];
        let mut stream = sqlx::query_with(&sql, args).fetch(&self.pool);

        while let Some(row) = stream.try_next().await? {
            let time: i64 = row.try_get("bucket_time")?;
            let count: i64 = row.try_get("count")?;
            let debug = DateTime::from_seconds(time);

            // Fill in gaps with zero counts
            while next_time < time {
                let dt = DateTime::from_seconds(next_time);
                results.push(Element {
                    time: next_time * 1000,
                    count: 0,
                    debug: dt.to_eve(),
                });
                next_time += interval as i64;
            }

            results.push(Element {
                time: time * 1000,
                count: count as u64,
                debug: debug.to_eve(),
            });
            next_time += interval as i64;
        }

        // Fill in remaining gaps up to now
        while next_time <= last_time {
            let dt = DateTime::from_seconds(next_time);
            results.push(Element {
                time: next_time * 1000,
                count: 0,
                debug: dt.to_eve(),
            });
            next_time += interval as i64;
        }

        let response: Vec<serde_json::Value> = results
            .iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect();

        Ok(response)
    }

    pub async fn get_event_by_id(&self, event_id: String) -> Result<Option<serde_json::Value>> {
        let id: i64 = match event_id.parse() {
            Ok(id) => id,
            Err(_) => return Ok(None),
        };

        let sql = r#"
            SELECT id, archived, escalated, source, history
            FROM events
            WHERE id = $1
        "#;

        let row = sqlx::query(sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let id: i64 = row.try_get("id")?;
            let archived: bool = row.try_get("archived")?;
            let escalated: bool = row.try_get("escalated")?;
            let mut parsed: serde_json::Value = row.try_get("source")?;
            let history: serde_json::Value = row.try_get("history")?;

            if let Some(timestamp) = parsed.get("timestamp") {
                let ts: serde_json::Value = timestamp.clone();
                parsed["@timestamp"] = ts;
            }

            if let serde_json::Value::Null = &parsed["tags"] {
                let tags: Vec<String> = Vec::new();
                parsed["tags"] = tags.into();
            }

            if let serde_json::Value::Array(tags) = &mut parsed["tags"] {
                if archived {
                    tags.push("evebox.archived".into());
                }
                if escalated {
                    tags.push("evebox.escalated".into());
                }
            }

            ensure_has_history(&mut parsed);
            parsed["evebox"]["history"] = history;

            let event = json!({
                "_id": id,
                "_source": parsed,
            });
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<()> {
        let action = HistoryEntryBuilder::new_archived().build();
        let history_entry = serde_json::to_string(&action)?;

        let sql = r#"
            UPDATE events
            SET archived = TRUE,
                history = history || $1::jsonb
            WHERE id = $2
        "#;

        let id: i64 = event_id.parse()?;
        let n = sqlx::query(sql)
            .bind(&history_entry)
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if n == 0 {
            warn!("Archive by event ID request did not update any events");
            bail!("postgres: event not found");
        }
        Ok(())
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<()> {
        self.set_escalation_by_id(event_id, true).await
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<()> {
        self.set_escalation_by_id(event_id, false).await
    }

    async fn set_escalation_by_id(&self, event_id: &str, escalate: bool) -> Result<()> {
        let action = if escalate {
            HistoryEntryBuilder::new_escalate()
        } else {
            HistoryEntryBuilder::new_deescalate()
        }
        .build();
        let history_entry = serde_json::to_string(&action)?;

        let sql = r#"
            UPDATE events
            SET escalated = $1,
                history = history || $2::jsonb
            WHERE id = $3
        "#;

        let id: i64 = event_id.parse()?;
        let n = sqlx::query(sql)
            .bind(escalate)
            .bind(&history_entry)
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if n == 0 {
            warn!(
                "{}escalate by event ID request did not update any events",
                if escalate { "E" } else { "De-e" }
            );
            bail!("postgres: event not found");
        }
        Ok(())
    }

    /// Query alerts, grouping by signature_id, src_ip, and dest_ip.
    ///
    /// Similar to the SQLite implementation, this scans events ordered by timestamp
    /// descending and groups them in memory.
    #[instrument(skip_all)]
    pub async fn alerts(&self, options: AlertQueryOptions) -> Result<AlertsResult> {
        use chrono::Timelike;
        use indexmap::IndexMap;
        use std::collections::HashSet;
        use std::time::Instant;

        let mut builder = EventQueryBuilder::new();
        builder
            .select("id")
            .select("timestamp")
            .select("escalated")
            .select("archived")
            .select("source->>'host' AS host")
            .select("(source->'alert'->>'signature_id')::bigint AS alert_signature_id")
            .select("source->'alert'->>'signature' AS alert_signature")
            .select("(source->'alert'->>'severity')::bigint AS alert_severity")
            .select("source->'alert'->>'action' AS alert_action")
            .select("source->>'app_proto' AS app_proto")
            .select("source->>'dest_ip' AS dest_ip")
            .select("source->>'src_ip' AS src_ip")
            .select("source->'tags' AS tags")
            .select("source->'dns' AS dns")
            .select("source->'tls' AS tls")
            .select("source->'quic' AS quic")
            .select("source->'http'->>'hostname' AS http_hostname");
        builder.from("events");
        builder.order_by("timestamp", "DESC");

        // Filter for alert events
        let p = builder.next_placeholder();
        builder
            .push_where(format!("source->>'event_type' = {}", p))
            .push_arg("alert".to_string())?;

        // Handle tags filter (archived, escalated)
        for tag in &options.tags {
            match tag.as_ref() {
                "evebox.archived" => {
                    let p = builder.next_placeholder();
                    builder
                        .push_where(format!("archived = {}", p))
                        .push_arg(true)?;
                }
                "-evebox.archived" => {
                    let p = builder.next_placeholder();
                    builder
                        .push_where(format!("archived = {}", p))
                        .push_arg(false)?;
                }
                "evebox.escalated" => {
                    let p = builder.next_placeholder();
                    builder
                        .push_where(format!("escalated = {}", p))
                        .push_arg(true)?;
                }
                _ => {}
            }
        }

        // Handle sensor filter
        if let Some(sensor) = &options.sensor {
            if sensor == "(no-name)" {
                builder.push_where("source->>'host' IS NULL");
            } else {
                let p = builder.next_placeholder();
                builder
                    .push_where(format!("source->>'host' = {}", p))
                    .push_arg(sensor.to_string())?;
            }
        }

        // Handle timestamp filter
        if let Some(ts) = &options.timestamp_gte {
            builder.timestamp_gte(ts)?;
        }

        // Handle query string
        if let Some(query_string) = &options.query_string {
            match queryparser::parse(query_string, None) {
                Err(err) => {
                    error!(
                        "Failed to parse query string: error={}, query string={}",
                        &err, &query_string
                    );
                }
                Ok(elements) => {
                    for el in &elements {
                        match &el.value {
                            queryparser::QueryValue::String(s) => {
                                let p = builder.next_placeholder();
                                if el.negated {
                                    builder
                                        .push_where(format!(
                                            "NOT source_vector @@ plainto_tsquery('simple', {})",
                                            p
                                        ))
                                        .push_arg(s.to_string())?;
                                } else {
                                    builder
                                        .push_where(format!(
                                            "source_vector @@ plainto_tsquery('simple', {})",
                                            p
                                        ))
                                        .push_arg(s.to_string())?;
                                }
                            }
                            queryparser::QueryValue::KeyValue(k, v) => {
                                let k = match k.as_ref() {
                                    "@sid" => "alert.signature_id",
                                    "@sig" => "alert.signature",
                                    _ => k,
                                };

                                if let Ok(num) = v.parse::<i64>() {
                                    let op = if el.negated { "!=" } else { "=" };
                                    // Build JSON path for numeric comparison
                                    // Use ->> for the final accessor to get text, then cast to bigint
                                    if k.contains('.') {
                                        let parts: Vec<&str> = k.split('.').collect();
                                        let mut path = "source".to_string();
                                        for (i, part) in parts.iter().enumerate() {
                                            if i == parts.len() - 1 {
                                                path.push_str(&format!("->>'{}'", part));
                                            } else {
                                                path.push_str(&format!("->'{}'", part));
                                            }
                                        }
                                        let p = builder.next_placeholder();
                                        builder
                                            .push_where(format!("({})::bigint {} {}", path, op, p))
                                            .push_arg(num)?;
                                    } else {
                                        let p = builder.next_placeholder();
                                        builder
                                            .push_where(format!(
                                                "(source->>'{}')::bigint {} {}",
                                                k, op, p
                                            ))
                                            .push_arg(num)?;
                                    }
                                } else {
                                    let op = if el.negated { "NOT ILIKE" } else { "ILIKE" };
                                    // Build JSON path for string comparison
                                    if k.contains('.') {
                                        let parts: Vec<&str> = k.split('.').collect();
                                        let mut path = "source".to_string();
                                        for (i, part) in parts.iter().enumerate() {
                                            if i == parts.len() - 1 {
                                                path.push_str(&format!("->>'{}' ", part));
                                            } else {
                                                path.push_str(&format!("->'{}'", part));
                                            }
                                        }
                                        let p = builder.next_placeholder();
                                        builder
                                            .push_where(format!("{} {} {}", path.trim(), op, p))
                                            .push_arg(format!("%{}%", v))?;
                                    } else {
                                        let p = builder.next_placeholder();
                                        builder
                                            .push_where(format!("source->>'{}' {} {}", k, op, p))
                                            .push_arg(format!("%{}%", v))?;
                                    }
                                }
                            }
                            queryparser::QueryValue::From(dt) => {
                                builder.timestamp_gte(dt)?;
                            }
                            queryparser::QueryValue::To(dt) => {
                                builder.timestamp_lte(dt)?;
                            }
                            queryparser::QueryValue::After(dt) => {
                                builder.timestamp_gt(dt)?;
                            }
                            queryparser::QueryValue::Before(dt) => {
                                builder.timestamp_lt(dt)?;
                            }
                        }
                    }
                }
            }
        }

        let (sql, args) = builder.build()?;

        debug!("Alerts query: {}", &sql);

        // Track sensors
        let mut sensors: HashSet<String> = HashSet::new();

        // Track the time range
        let mut max_timestamp = None;
        let mut min_timestamp = None;

        // Only enforce timeout if explicitly set (matching SQLite semantics)
        let timeout = options.timeout;

        let mut events: IndexMap<String, AggAlert> = IndexMap::new();
        let mut rows = sqlx::query_with(&sql, args).fetch(&self.pool);
        let mut now = Instant::now();
        let mut timed_out = false;
        let mut count = 0;
        let mut abort_at = None;

        while let Some(row) = rows.try_next().await? {
            // The columns that make up the key
            let alert_signature_id: i64 = row.try_get("alert_signature_id")?;
            let src_ip: Option<String> = row.try_get("src_ip")?;
            let dest_ip: Option<String> = row.try_get("dest_ip")?;

            let escalated: bool = row.try_get("escalated")?;
            let host: Option<String> = row.try_get("host").unwrap_or(None);

            let id: i64 = row.try_get("id")?;
            let timestamp: chrono::DateTime<chrono::Utc> = row.try_get("timestamp")?;

            // If timed-out, keep processing events in this second
            if timed_out {
                let abort_at = abort_at.unwrap();
                if timestamp < abort_at {
                    break;
                }
            }

            if max_timestamp.is_none() {
                max_timestamp = Some(timestamp);
            }
            min_timestamp = Some(timestamp);

            if let Some(host) = &host {
                sensors.insert(host.to_string());
            }

            let key = format!(
                "{}{}{}",
                alert_signature_id,
                src_ip.as_deref().unwrap_or(""),
                dest_ip.as_deref().unwrap_or("")
            );

            if let Some(entry) = events.get_mut(&key) {
                entry.metadata.count += 1;
                if escalated {
                    entry.metadata.escalated_count += 1;
                }
                entry.metadata.min_timestamp = DateTime::from(timestamp);
            } else {
                // Get the columns needed to construct a new entry
                let quic: serde_json::Value =
                    row.try_get("quic").unwrap_or(serde_json::Value::Null);
                let http_hostname: Option<String> = row.try_get("http_hostname")?;
                let tls: serde_json::Value = row.try_get("tls").unwrap_or(serde_json::Value::Null);
                let dns: serde_json::Value = row.try_get("dns").unwrap_or(serde_json::Value::Null);
                let archived: bool = row.try_get("archived")?;
                let alert_signature: String = row.try_get("alert_signature")?;
                let alert_severity: i64 = row.try_get("alert_severity")?;
                let alert_action: String = row.try_get("alert_action")?;
                let app_proto: Option<String> = row.try_get("app_proto")?;
                let tags: serde_json::Value =
                    row.try_get("tags").unwrap_or(serde_json::Value::Null);

                let mut source = json!({
                    "timestamp": DateTime::from(timestamp).to_eve(),
                    "tags": tags,
                    "dest_ip": dest_ip,
                    "src_ip": src_ip,
                    "app_proto": app_proto,
                    "host": host,
                    "alert": {
                        "signature": alert_signature,
                        "signature_id": alert_signature_id,
                        "severity": alert_severity,
                        "action": alert_action,
                    },
                    "tls": tls,
                    "dns": dns,
                    "quic": quic,
                });

                if let Some(http_hostname) = http_hostname {
                    source["http"]["hostname"] = http_hostname.into();
                }

                if let serde_json::Value::Null = &source["tags"] {
                    let tags: Vec<String> = Vec::new();
                    source["tags"] = tags.into();
                }

                if let serde_json::Value::Array(tags) = &mut source["tags"] {
                    if archived && !tags.contains(&"evebox.archived".into()) {
                        tags.push("evebox.archived".into());
                    }
                }

                let alert = AggAlert {
                    id: id.to_string(),
                    source,
                    metadata: AggAlertMetadata {
                        count: 1,
                        escalated_count: if escalated { 1 } else { 0 },
                        min_timestamp: DateTime::from(timestamp),
                        max_timestamp: DateTime::from(timestamp),
                    },
                };
                events.insert(key, alert);
            }

            if count == 0 {
                debug!("First row took {:?}", now.elapsed());
                // Reset timer after first result
                now = Instant::now();
            }

            count += 1;

            if let Some(timeout) = timeout {
                if now.elapsed() > std::time::Duration::from_secs(timeout) {
                    timed_out = true;
                    abort_at = timestamp.with_nanosecond(0);
                }
            }
        }

        let took = now.elapsed();

        let min_timestamp = min_timestamp.map(DateTime::from);
        let max_timestamp = max_timestamp.map(DateTime::from);

        if timed_out {
            info!(
                ?timed_out,
                "Alert query took {:?}, with {} events over {} groups",
                &took,
                count,
                events.len()
            );
        }

        let results: Vec<AggAlert> = events.into_values().collect();

        Ok(AlertsResult {
            events: results,
            timed_out,
            took: took.as_millis() as u64,
            ecs: false,
            min_timestamp,
            max_timestamp,
        })
    }

    pub async fn archive_by_alert_group(&self, alert_group: AlertGroupSpec) -> Result<u64> {
        debug!("Archiving alert group: {:?}", alert_group);

        let action = HistoryEntryBuilder::new_archived().build();
        let history_entry = serde_json::to_string(&action)?;

        let mut args = PgArguments::default();
        let mut filters: Vec<String> = Vec::new();

        macro_rules! add_arg {
            ($val:expr) => {
                args.add($val).map_err(sqlx::Error::Encode)?
            };
        }

        // $1: history entry as JSON
        add_arg!(&history_entry);

        // Filter for alert events that are not already archived
        filters.push("source->>'event_type' = 'alert'".to_string());
        filters.push("archived = FALSE".to_string());

        // $2: signature_id
        add_arg!(alert_group.signature_id as i64);
        filters.push("(source->'alert'->>'signature_id')::bigint = $2".to_string());

        // Track placeholder number
        let mut next_placeholder = 3;

        // src_ip (handle NULL case)
        if let Some(src_ip) = &alert_group.src_ip {
            if src_ip.is_empty() {
                filters.push("(source->>'src_ip' IS NULL OR source->>'src_ip' = '')".to_string());
            } else {
                add_arg!(src_ip);
                filters.push(format!("source->>'src_ip' = ${}", next_placeholder));
                next_placeholder += 1;
            }
        } else {
            filters.push("(source->>'src_ip' IS NULL OR source->>'src_ip' = '')".to_string());
        }

        // dest_ip (handle NULL case)
        if let Some(dest_ip) = &alert_group.dest_ip {
            if dest_ip.is_empty() {
                filters.push("(source->>'dest_ip' IS NULL OR source->>'dest_ip' = '')".to_string());
            } else {
                add_arg!(dest_ip);
                filters.push(format!("source->>'dest_ip' = ${}", next_placeholder));
                next_placeholder += 1;
            }
        } else {
            filters.push("(source->>'dest_ip' IS NULL OR source->>'dest_ip' = '')".to_string());
        }

        // min_timestamp
        let mints = crate::datetime::parse(&alert_group.min_timestamp, None)?;
        add_arg!(mints.datetime.to_utc());
        filters.push(format!("timestamp >= ${}", next_placeholder));
        next_placeholder += 1;

        // max_timestamp
        let maxts = crate::datetime::parse(&alert_group.max_timestamp, None)?;
        add_arg!(maxts.datetime.to_utc());
        filters.push(format!("timestamp <= ${}", next_placeholder));

        let sql = format!(
            "UPDATE events SET archived = TRUE, history = history || $1::jsonb WHERE {}",
            filters.join(" AND ")
        );

        debug!("Archive query: {}", &sql);

        let start = Instant::now();
        let result = sqlx::query_with(&sql, args).execute(&self.pool).await?;
        let n = result.rows_affected();
        info!("Archived {} alerts in {:?}", n, start.elapsed());
        Ok(n)
    }

    async fn set_escalation_by_alert_group(
        &self,
        session: Arc<Session>,
        alert_group: AlertGroupSpec,
        escalate: bool,
    ) -> Result<u64> {
        let action = if escalate {
            HistoryEntryBuilder::new_escalate()
        } else {
            HistoryEntryBuilder::new_deescalate()
        }
        .username(session.username.clone())
        .build();

        let history_entry = serde_json::to_string(&action)?;

        // Build parameterized query
        let mut args = PgArguments::default();
        let mut filters: Vec<String> = Vec::new();

        // Helper closure to add args with proper error conversion
        macro_rules! add_arg {
            ($val:expr) => {
                args.add($val).map_err(sqlx::Error::Encode)?
            };
        }

        // $1: escalated value (true or false)
        add_arg!(escalate);
        // $2: history entry as JSON
        add_arg!(&history_entry);

        // Filter for alert events
        filters.push("source->>'event_type' = 'alert'".to_string());

        // Filter for events not already in the target state
        // $3: current escalated value (false if escalating, true if deescalating)
        add_arg!(!escalate);
        filters.push("escalated = $3".to_string());

        // $4: signature_id
        add_arg!(alert_group.signature_id as i64);
        filters.push("(source->'alert'->>'signature_id')::bigint = $4".to_string());

        // $5: src_ip
        add_arg!(&alert_group.src_ip);
        if alert_group.src_ip.is_some() {
            filters.push("source->>'src_ip' = $5".to_string());
        } else {
            filters.push("source->>'src_ip' IS NULL".to_string());
        }

        // $6: dest_ip
        add_arg!(&alert_group.dest_ip);
        if alert_group.dest_ip.is_some() {
            filters.push("source->>'dest_ip' = $6".to_string());
        } else {
            filters.push("source->>'dest_ip' IS NULL".to_string());
        }

        // $7: min_timestamp
        let mints = crate::datetime::parse(&alert_group.min_timestamp, None)?;
        add_arg!(mints.datetime.to_utc());
        filters.push("timestamp >= $7".to_string());

        // $8: max_timestamp
        let maxts = crate::datetime::parse(&alert_group.max_timestamp, None)?;
        add_arg!(maxts.datetime.to_utc());
        filters.push("timestamp <= $8".to_string());

        let sql = format!(
            "UPDATE events SET escalated = $1, history = history || $2::jsonb WHERE {}",
            filters.join(" AND ")
        );

        debug!("Escalation query: {}", &sql);

        let start = Instant::now();
        let result = sqlx::query_with(&sql, args).execute(&self.pool).await?;
        let n = result.rows_affected();
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
        session: Arc<Session>,
        alert_group: AlertGroupSpec,
    ) -> Result<()> {
        let n = self
            .set_escalation_by_alert_group(session, alert_group, true)
            .await?;
        debug!("Escalated {n} alerts in group");
        Ok(())
    }

    pub async fn deescalate_by_alert_group(
        &self,
        session: Arc<Session>,
        alert_group: AlertGroupSpec,
    ) -> Result<()> {
        let n = self
            .set_escalation_by_alert_group(session, alert_group, false)
            .await?;
        debug!("De-escalated {n} alerts in group");
        Ok(())
    }

    pub async fn events(&self, params: EventQueryParams) -> Result<serde_json::Value> {
        let mut builder = EventQueryBuilder::new();
        builder
            .select("id")
            .select("archived")
            .select("escalated")
            .select("source");
        builder.from("events");

        if let Some(event_type) = params.event_type {
            builder.where_source_json("event_type", "=", &event_type)?;
        }

        if let Some(dt) = &params.to {
            builder.timestamp_lte(dt)?;
        }

        if let Some(dt) = &params.from {
            builder.timestamp_gte(dt)?;
        }

        builder.apply_query_string(&params.query_string)?;

        if let Some(order) = &params.order {
            builder.order_by("timestamp", order);
        } else {
            builder.order_by("timestamp", "DESC");
        }

        builder.limit(params.size.unwrap_or(500) as i64);

        let (sql, args) = builder.build()?;

        let mut rows = sqlx::query_with(&sql, args).fetch(&self.pool);
        let mut events = Vec::new();

        while let Some(row) = rows.try_next().await? {
            let mut event = row_mapper(row)?;

            if let Some(ja4) = event["_source"]["tls"]["ja4"].as_str() {
                if let Some(configdb) = crate::server::context::get_configdb() {
                    let sql = "SELECT data FROM ja4db WHERE fingerprint = ?";
                    let info: Result<Option<serde_json::Value>, _> = sqlx::query_scalar(sql)
                        .bind(ja4)
                        .fetch_optional(&configdb.pool)
                        .await;
                    if let Ok(Some(info)) = info {
                        event["_source"]["ja4db"] = info;
                    }
                }
            }

            events.push(event);
        }

        Ok(json!({
            "ecs": false,
            "events": events
        }))
    }

    pub async fn comment_event_by_id(
        &self,
        event_id: &str,
        comment: String,
        session: Arc<Session>,
    ) -> Result<()> {
        let action = HistoryEntryBuilder::new_comment()
            .username(session.username.clone())
            .comment(comment)
            .build();
        let history_entry = serde_json::to_string(&action)?;

        let sql = r#"
            UPDATE events
            SET history = history || $1::jsonb
            WHERE id = $2
        "#;

        let id: i64 = event_id.parse()?;
        let n = sqlx::query(sql)
            .bind(&history_entry)
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if n == 0 {
            warn!("Comment by event ID request did not update any events");
            bail!("postgres: event not found");
        }
        Ok(())
    }

    /// Maximum time range (in hours) for expensive flow aggregation queries.
    /// Flow events typically dominate event volume, so we limit the time range
    /// to prevent extremely slow queries.
    const MAX_FLOW_AGG_HOURS: i64 = 6;

    /// Fields that are expensive to aggregate due to high event volume.
    /// These are typically flow-related fields.
    const EXPENSIVE_AGG_FIELDS: &'static [&'static str] =
        &["src_port", "dest_port", "proto", "src_ip", "dest_ip"];

    pub async fn agg(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<queryparser::QueryElement>,
    ) -> Result<Vec<serde_json::Value>> {
        // For expensive fields, limit the time range to prevent slow queries
        let query = self.apply_agg_time_limit(field, query);

        let mut builder = EventQueryBuilder::new();

        if field == "dns.rrname" {
            let coa = "COALESCE(source->'dns'->'queries'->0->>'rrname', source->'dns'->>'rrname')";
            builder.select("COUNT(*) AS count".to_string());
            builder.select(format!("{coa} AS agg"));
            // Filter out NULL values
            builder.push_where(format!("{coa} IS NOT NULL"));
        } else {
            // Build the JSON path for the field
            let json_path = Self::field_to_json_path(field);
            builder.select("COUNT(*) AS count".to_string());
            builder.select(format!("{json_path} AS agg"));
            // Filter out NULL values
            builder.push_where(format!("{json_path} IS NOT NULL"));
        }

        builder.from("events");
        builder.apply_query_string(&query)?;

        // Add event_type optimization for common field prefixes
        if field.starts_with("alert.") {
            let p = builder.next_placeholder();
            builder
                .push_where(format!("source->>'event_type' = {}", p))
                .push_arg("alert".to_string())?;
        } else if field.starts_with("dns.") {
            let p = builder.next_placeholder();
            builder
                .push_where(format!("source->>'event_type' = {}", p))
                .push_arg("dns".to_string())?;
        }

        let (base_sql, args) = builder.build()?;

        // Add GROUP BY, ORDER BY, and LIMIT
        let order_dir = if order == "asc" { "ASC" } else { "DESC" };
        let sql = format!(
            "{} GROUP BY agg ORDER BY count {} LIMIT {}",
            base_sql, order_dir, size
        );

        debug!("Agg query: {}", &sql);

        let mut results = vec![];
        let mut rows = sqlx::query_with(&sql, args).fetch(&self.pool);

        while let Some(row) = rows.try_next().await? {
            let count: i64 = row.try_get("count")?;
            // Decode as Option to handle any edge cases with NULL values
            let val: Option<String> = row.try_get("agg")?;
            if let Some(val) = val {
                results.push(json!({"count": count, "key": val}));
            }
        }

        Ok(results)
    }

    /// Helper to convert a dotted field path to PostgreSQL JSON path syntax.
    fn field_to_json_path(field: &str) -> String {
        let parts: Vec<&str> = field.split('.').collect();
        let mut path = "source".to_string();
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                path.push_str(&format!("->>'{}' ", part));
            } else {
                path.push_str(&format!("->'{}' ", part));
            }
        }
        path.trim().to_string()
    }

    /// Apply time range limits for expensive aggregation queries.
    ///
    /// For flow-related fields, the time range is limited to MAX_FLOW_AGG_HOURS
    /// to prevent extremely slow queries on high-volume data.
    fn apply_agg_time_limit(
        &self,
        field: &str,
        mut query: Vec<queryparser::QueryElement>,
    ) -> Vec<queryparser::QueryElement> {
        // Only apply limits to expensive fields
        if !Self::EXPENSIVE_AGG_FIELDS.contains(&field) {
            return query;
        }

        let now = DateTime::now();
        let max_duration = chrono::Duration::hours(Self::MAX_FLOW_AGG_HOURS);
        let min_allowed = DateTime::from((now.datetime - max_duration).fixed_offset());

        // Find the existing "from" timestamp in the query
        let mut from_idx = None;
        let mut current_from: Option<DateTime> = None;

        for (i, element) in query.iter().enumerate() {
            if let queryparser::QueryValue::From(ts) = &element.value {
                from_idx = Some(i);
                current_from = Some(ts.clone());
                break;
            }
        }

        // Check if the current range exceeds the limit
        if let Some(from_ts) = current_from {
            if from_ts < min_allowed {
                info!(
                    "Limiting aggregation time range for field '{}' from {} to {} (max {} hours)",
                    field,
                    from_ts,
                    min_allowed,
                    Self::MAX_FLOW_AGG_HOURS
                );

                // Replace the existing from timestamp with the limited one
                if let Some(idx) = from_idx {
                    query[idx] = queryparser::QueryElement {
                        negated: false,
                        value: queryparser::QueryValue::From(min_allowed),
                    };
                }
            }
        } else {
            // No from timestamp, add one
            debug!(
                "Adding default time limit for expensive aggregation field '{}'",
                field
            );
            query.push(queryparser::QueryElement {
                negated: false,
                value: queryparser::QueryValue::From(min_allowed),
            });
        }

        query
    }

    async fn get_stats(&self, params: &StatsAggQueryParams) -> Result<Vec<(i64, i64)>> {
        let start_time = params.start_time.datetime.to_utc();
        let end_time = params.end_time.datetime.to_utc();
        let range = (params.end_time.datetime - params.start_time.datetime).num_seconds();
        let interval = crate::util::histogram_interval(range);

        let mut args = PgArguments::default();
        let path: Vec<String> = params.field.split('.').map(|s| s.to_string()).collect();

        // $1: Path
        args.add(&path).map_err(sqlx::Error::Encode)?;

        let mut filters = vec![
            "source->>'event_type' = 'stats'".to_string(),
            "timestamp >= $2".to_string(),
            "timestamp <= $3".to_string(),
        ];

        // $2: Start time
        args.add(start_time).map_err(sqlx::Error::Encode)?;
        // $3: End time
        args.add(end_time).map_err(sqlx::Error::Encode)?;

        if let Some(sensor_name) = &params.sensor_name {
            if sensor_name == "(no-name)" {
                filters.push("source->>'host' IS NULL".to_string());
            } else {
                filters.push("source->>'host' = $4".to_string());
                args.add(sensor_name).map_err(sqlx::Error::Encode)?;
            }
        }

        let sql = format!(
            r#"
            SELECT
              (EXTRACT(EPOCH FROM timestamp)::bigint / {interval}) * {interval} AS bucket_time,
              MAX((source #>> $1)::bigint)
            FROM events
            WHERE {filters}
            GROUP BY bucket_time
            ORDER BY bucket_time
            "#,
            interval = interval,
            filters = filters.join(" AND ")
        );

        debug!("Stats agg query: {}", &sql);

        let start = Instant::now();
        let rows: Vec<(i64, Option<i64>)> = sqlx::query_as_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        debug!(
            "Returning {} stats records in {} ms",
            rows.len(),
            start.elapsed().as_millis()
        );

        Ok(rows
            .into_iter()
            .map(|(ts, val)| (ts, val.unwrap_or(0)))
            .collect())
    }

    pub async fn stats_agg(&self, params: &StatsAggQueryParams) -> Result<serde_json::Value> {
        let rows = self.get_stats(params).await?;
        let response_data: Vec<serde_json::Value> = rows
            .iter()
            .map(|(timestamp, value)| {
                json!({
                    "value": value,
                    "timestamp": DateTime::from_seconds(*timestamp).to_rfc3339_utc(),
                })
            })
            .collect();
        Ok(json!({
            "data": response_data,
        }))
    }

    pub async fn stats_agg_diff(&self, params: &StatsAggQueryParams) -> Result<serde_json::Value> {
        let rows = self.get_stats(params).await?;
        let mut response_data = vec![];
        for (i, e) in rows.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let previous = rows[i - 1].1;
            let value = if previous <= e.1 { e.1 - previous } else { e.1 };
            response_data.push(json!({
                "value": value,
                "timestamp": DateTime::from_seconds(e.0).to_rfc3339_utc(),
            }));
        }
        Ok(json!({
            "data": response_data,
        }))
    }

    pub async fn stats_agg_by_sensor(
        &self,
        params: &StatsAggQueryParams,
    ) -> Result<serde_json::Value> {
        let start_time = params.start_time.datetime.to_utc();
        let end_time = params.end_time.datetime.to_utc();
        let range = (params.end_time.datetime - params.start_time.datetime).num_seconds();
        let interval = crate::util::histogram_interval(range);

        let mut args = PgArguments::default();

        let path: Vec<String> = params.field.split('.').map(|s| s.to_string()).collect();

        // Use #>> operator to extract path as text, then cast to bigint for aggregation
        // We bind the path array as a parameter
        let sql = format!(
            r#"
            SELECT
              source->>'host' AS sensor,
              (EXTRACT(EPOCH FROM timestamp)::bigint / {interval}) * {interval} AS bucket_time,
              MAX((source #>> $1)::bigint)
            FROM events
            WHERE source->>'event_type' = 'stats'
              AND timestamp >= $2
              AND timestamp <= $3
            GROUP BY sensor, bucket_time
            ORDER BY sensor, bucket_time
            "#
        );

        args.add(&path).map_err(sqlx::Error::Encode)?;
        args.add(start_time).map_err(sqlx::Error::Encode)?;
        args.add(end_time).map_err(sqlx::Error::Encode)?;

        debug!("Stats agg query: {}", &sql);

        let start = Instant::now();
        let rows: Vec<(Option<String>, i64, Option<i64>)> = sqlx::query_as_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        debug!(
            "Returning {} stats records by sensor in {} ms",
            rows.len(),
            start.elapsed().as_millis()
        );

        let mut sensor_data: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

        for (sensor, timestamp, value) in rows {
            let sensor_name = sensor.unwrap_or_else(|| "(no-name)".to_string());
            let entry = json!({
                "timestamp": DateTime::from_seconds(timestamp).to_rfc3339_utc(),
                "value": value.unwrap_or(0),
            });
            sensor_data.entry(sensor_name).or_default().push(entry);
        }

        Ok(json!({
            "data": sensor_data,
        }))
    }

    pub async fn stats_agg_diff_by_sensor(
        &self,
        params: &StatsAggQueryParams,
    ) -> Result<serde_json::Value> {
        let start_time = params.start_time.datetime.to_utc();
        let end_time = params.end_time.datetime.to_utc();
        let range = (params.end_time.datetime - params.start_time.datetime).num_seconds();
        let interval = crate::util::histogram_interval(range);

        let mut args = PgArguments::default();

        let path: Vec<String> = params.field.split('.').map(|s| s.to_string()).collect();

        let sql = format!(
            r#"
            SELECT
              source->>'host' AS sensor,
              (EXTRACT(EPOCH FROM timestamp)::bigint / {interval}) * {interval} AS bucket_time,
              MAX((source #>> $1)::bigint)
            FROM events
            WHERE source->>'event_type' = 'stats'
              AND timestamp >= $2
              AND timestamp <= $3
            GROUP BY sensor, bucket_time
            ORDER BY sensor, bucket_time
            "#
        );

        args.add(&path).map_err(sqlx::Error::Encode)?;
        args.add(start_time).map_err(sqlx::Error::Encode)?;
        args.add(end_time).map_err(sqlx::Error::Encode)?;

        debug!("Stats agg diff query: {}", &sql);

        let start = Instant::now();
        let rows: Vec<(Option<String>, i64, Option<i64>)> = sqlx::query_as_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        debug!(
            "Returning {} stats diff records by sensor in {} ms",
            rows.len(),
            start.elapsed().as_millis()
        );

        let mut sensor_data: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
        let mut previous_values: HashMap<String, i64> = HashMap::new();

        for (sensor, timestamp, value) in rows {
            let sensor_name = sensor.unwrap_or_else(|| "(no-name)".to_string());
            let value = value.unwrap_or(0);

            if let Some(&previous) = previous_values.get(&sensor_name) {
                let diff_value = if previous <= value {
                    value - previous
                } else {
                    value
                };
                let entry = json!({
                    "timestamp": DateTime::from_seconds(timestamp).to_rfc3339_utc(),
                    "value": diff_value,
                });
                sensor_data
                    .entry(sensor_name.clone())
                    .or_default()
                    .push(entry);
            }
            previous_values.insert(sensor_name, value);
        }

        Ok(json!({
            "data": sensor_data,
        }))
    }

    /// DNS reverse lookup: find hostnames that resolved to the given IP address.
    ///
    /// Looks for DNS events where the answers contain an IP matching `src_ip`,
    /// returning the distinct rrnames (resolved names) from those events.
    pub async fn dns_reverse_lookup(
        &self,
        before: Option<DateTime>,
        sensor: Option<String>,
        src_ip: String,
        dest_ip: String,
    ) -> Result<serde_json::Value> {
        // If before is None, set to now.
        let before = before.unwrap_or_else(DateTime::now);

        // Set after to 1 hour before "before".
        let after: DateTime = (before.datetime - chrono::Duration::hours(1)).into();

        let mut builder = EventQueryBuilder::new();
        builder.select(
            "DISTINCT COALESCE(source->'dns'->'queries'->0->>'rrname', source->'dns'->>'rrname')",
        );
        builder.from("events");
        builder.from("jsonb_array_elements(source->'dns'->'answers') AS answers");

        builder.timestamp_lte(&before)?;
        builder.timestamp_gte(&after)?;

        // Filter for DNS events
        let p = builder.next_placeholder();
        builder
            .push_where(format!("source->>'event_type' = {}", p))
            .push_arg("dns".to_string())?;

        // Handle sensor filter
        if let Some(host) = sensor {
            if host == "(no-name)" {
                builder.push_where("source->>'host' IS NULL");
            } else {
                let p = builder.next_placeholder();
                builder
                    .push_where(format!("source->>'host' = {}", p))
                    .push_arg(host)?;
            }
        }

        // Filter for src_ip/dest_ip matching either provided IP
        let p1 = builder.next_placeholder();
        let p2 = builder.next_placeholder();
        let p3 = builder.next_placeholder();
        let p4 = builder.next_placeholder();
        builder.push_where(format!(
            "(source->>'dest_ip' = {} OR source->>'dest_ip' = {} OR source->>'src_ip' = {} OR source->>'src_ip' = {})",
            p1, p2, p3, p4
        ));
        builder.push_arg(src_ip.clone())?;
        builder.push_arg(dest_ip.clone())?;
        builder.push_arg(src_ip.clone())?;
        builder.push_arg(dest_ip)?;

        // Filter for answers with rdata matching src_ip
        let p = builder.next_placeholder();
        builder
            .push_where(format!("answers->>'rdata' = {}", p))
            .push_arg(src_ip)?;

        // Limit to responses
        builder.push_where(
            "(source->'dns'->>'type' = 'response' OR source->'dns'->>'type' = 'answer')",
        );

        // Note: No ORDER BY here since we're using DISTINCT and just want a list of rrnames

        let (sql, args) = builder.build()?;
        debug!("DNS reverse lookup query: {}", &sql);

        let mut rrnames = vec![];
        let mut rows = sqlx::query_scalar_with::<_, String, _>(&sql, args).fetch(&self.pool);
        while let Some(rrname) = rows.try_next().await? {
            rrnames.push(rrname);
        }

        Ok(json!({
            "rrnames": rrnames,
        }))
    }

    /// Get distinct event types from events matching the query.
    pub async fn get_event_types(
        &self,
        query: Vec<queryparser::QueryElement>,
    ) -> Result<Vec<String>> {
        let mut builder = EventQueryBuilder::new();
        builder.select("DISTINCT source->>'event_type'");
        builder.from("events");
        builder.apply_query_string(&query)?;

        let (sql, args) = builder.build()?;

        let rows: Vec<String> = sqlx::query_scalar_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }

    /// Get distinct sensor names from events in the last 24 hours.
    ///
    /// Returns a sorted list of sensor names, with "(no-name)" at the end
    /// for events that don't have a host field.
    pub async fn get_sensors(&self) -> Result<Vec<String>> {
        let sql = r#"
            SELECT DISTINCT source->>'host' AS host
            FROM events
            WHERE timestamp >= $1
            "#;

        let cutoff = (DateTime::now().datetime - chrono::Duration::hours(24)).to_utc();

        let rows: Vec<Option<String>> = sqlx::query_scalar(sql)
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;

        // Map NULL values to "(no-name)"
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

    /// Query DHCP events, returning the most recent event per client MAC.
    pub async fn dhcp(
        &self,
        earliest: Option<DateTime>,
        dhcp_type: &str,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut builder = EventQueryBuilder::new();

        // Select the full source from the most recent event per client MAC
        builder.select("t1.source");
        builder.from("events t1");

        // Build the subquery to get the max timestamp per client MAC
        let mut subquery_wheres = vec![
            "source->>'event_type' = 'dhcp'".to_string(),
            format!("source->'dhcp'->>'dhcp_type' = '{}'", dhcp_type),
        ];

        let mut subquery_params = PgArguments::default();
        let mut param_idx = 1;

        if let Some(earliest) = earliest {
            subquery_wheres.push(format!("timestamp >= ${}", param_idx));
            subquery_params
                .add(earliest.datetime.to_utc())
                .map_err(sqlx::Error::Encode)?;
            param_idx += 1;
        }

        if let Some(sensor) = &sensor {
            if sensor == "(no-name)" {
                subquery_wheres.push("source->>'host' IS NULL".to_string());
            } else {
                subquery_wheres.push(format!("source->>'host' = ${}", param_idx));
                subquery_params
                    .add(sensor.to_string())
                    .map_err(sqlx::Error::Encode)?;
            }
        }

        let subquery = format!(
            r#"
            SELECT MAX(timestamp) AS timestamp,
                   source->'dhcp'->>'client_mac' AS dhcp_client_mac
            FROM events
            WHERE {}
            GROUP BY source->'dhcp'->>'client_mac'
            "#,
            subquery_wheres.join(" AND ")
        );

        let sql = format!(
            r#"
            SELECT t1.source
            FROM events t1
            JOIN ({}) t2
            ON t1.timestamp = t2.timestamp
               AND t1.source->'dhcp'->>'client_mac' = t2.dhcp_client_mac
            WHERE t1.source->>'event_type' = 'dhcp'
            "#,
            subquery
        );

        debug!("DHCP query: {}", &sql);

        let mut rows = sqlx::query_with(&sql, subquery_params).fetch(&self.pool);
        let mut events = vec![];

        while let Some(row) = rows.try_next().await? {
            let source: serde_json::Value = row.try_get("source")?;
            events.push(source);
        }

        Ok(events)
    }

    /// Query DHCP request events.
    pub async fn dhcp_request(
        &self,
        earliest: Option<DateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        self.dhcp(earliest, "request", sensor).await
    }

    /// Query DHCP ack events.
    pub async fn dhcp_ack(
        &self,
        earliest: Option<DateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        self.dhcp(earliest, "ack", sensor).await
    }
}

fn row_mapper(row: sqlx::postgres::PgRow) -> Result<serde_json::Value, sqlx::Error> {
    let id: i64 = row.try_get("id")?;
    let archived: bool = row.try_get("archived")?;
    let escalated: bool = row.try_get("escalated")?;
    let mut parsed: serde_json::Value = row.try_get("source")?;

    if let Some(timestamp) = parsed.get("timestamp") {
        let ts: serde_json::Value = timestamp.clone();
        parsed["@timestamp"] = ts;
    }

    if let serde_json::Value::Null = &parsed["tags"] {
        let tags: Vec<String> = Vec::new();
        parsed["tags"] = tags.into();
    }

    if let serde_json::Value::Array(tags) = &mut parsed["tags"] {
        if archived {
            tags.push("evebox.archived".into());
        }
        if escalated {
            tags.push("evebox.escalated".into());
        }
    }

    let event = json!({
        "_id": id,
        "_source": parsed,
    });
    Ok(event)
}
