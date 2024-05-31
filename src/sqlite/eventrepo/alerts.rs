// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use futures::TryStreamExt;
use indexmap::IndexMap;
use serde::Serialize;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::Arguments;
use sqlx::Row;
use tracing::{debug, error, info, instrument, warn};

use super::SqliteEventRepo;
use crate::datetime::DateTime;
use crate::server::api::{ApiAlertGroup, ApiAlertGroupMetadata, ApiAlertResponse};
use crate::sqlite::log_query_plan;
use crate::{elastic::AlertQueryOptions, eventrepo::DatastoreError};
use crate::{queryparser, LOG_QUERIES, LOG_QUERY_PLAN};
use std::time::Instant;

impl SqliteEventRepo {
    #[instrument(skip_all)]
    pub async fn _alerts_with_timeout(
        &self,
        options: AlertQueryOptions,
    ) -> Result<ApiAlertResponse, DatastoreError> {
        info!("_alerts_with_timeout");
        #[derive(Debug, Default, Serialize)]
        struct Element {
            rowid: String,
            source: serde_json::Value,
            count: u64,
            escalated_count: u64,
            min_timestamp: u64,
            max_timestamp: u64,
        }

        let mut filters: Vec<String> = Vec::new();
        let mut args = SqliteArguments::default();

        let sql = r#"
            SELECT rowid, timestamp, escalated, archived, source,
                json_extract(events.source, '$.alert.signature_id') as sid
            FROM events
            WHERE %WHERE%
            ORDER BY timestamp DESC"#;

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());

        for tag in options.tags {
            match tag.as_ref() {
                "evebox.archived" => {
                    filters.push("archived = ?".into());
                    args.add(1);
                }
                "-evebox.archived" => {
                    filters.push("archived = ?".into());
                    args.add(0);
                }
                "evebox.escalated" => {
                    filters.push("escalated = ?".into());
                    args.add(1);
                }
                _ => {}
            }
        }

        if let Some(sensor) = options.sensor {
            filters.push("json_extract(events.source, '$.host') = ?".into());
            args.add(sensor);
        }

        // TODO: With a timeout, we can remove this.
        if let Some(ts) = options.timestamp_gte {
            filters.push("timestamp >= ?".into());
            args.add(ts.to_nanos());
        }

        // Query string.
        if let Some(query_string) = options.query_string {
            match queryparser::parse(&query_string, None) {
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
                                if el.negated {
                                    filters.push("events.source NOT LIKE ?".into());
                                    args.add(format!("%{s}%"));
                                } else {
                                    filters.push("events.source LIKE ?".into());
                                    args.add(format!("%{s}%"));
                                }
                            }
                            queryparser::QueryValue::KeyValue(k, v) => {
                                // TODO: Handle negation - maybe use query builder?
                                if let Ok(v) = v.parse::<i64>() {
                                    filters
                                        .push(format!("json_extract(events.source, '$.{k}') = ?"));
                                    args.add(v);
                                } else {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{k}') LIKE ?"
                                    ));
                                    args.add(format!("%{v}%"));
                                }
                            }
                            queryparser::QueryValue::From(_) => {
                                warn!("QueryValue::From not supported here");
                            }
                            queryparser::QueryValue::To(_) => {
                                warn!("QueryValue::From not supported here");
                            }
                        }
                    }
                }
            }
        }

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &SqliteArguments::default()).await;
        } else if *LOG_QUERIES {
            info!(
                "query={}; args={:?}",
                &sql.trim(),
                &SqliteArguments::default()
            );
        }

        let mut events: IndexMap<String, ApiAlertGroup> = IndexMap::new();
        let mut rows = sqlx::query_with(&sql, args).fetch(&self.pool);
        let mut now = Instant::now();
        let mut timed_out = false;
        let mut count = 0;
        while let Some(row) = rows.try_next().await? {
            let rowid: u64 = row.try_get(0)?;
            let timestamp: u64 = row.try_get(1)?;
            let escalated: bool = row.try_get(2)?;
            let archived: bool = row.try_get(3)?;
            let mut source: serde_json::Value = row.try_get(4)?;
            let sid: u64 = row.try_get(5)?;

            let src_ip = source["src_ip"].as_str().unwrap().to_string();
            let dest_ip = source["dest_ip"].as_str().unwrap().to_string();

            let key = format!("{sid}{src_ip}{dest_ip}");

            if let serde_json::Value::Null = &source["tags"] {
                let tags: Vec<String> = Vec::new();
                source["tags"] = tags.into();
            }

            if let serde_json::Value::Array(ref mut tags) = &mut source["tags"] {
                if archived {
                    tags.push("evebox.archived".into());
                }
            }

            if let Some(entry) = events.get_mut(&key) {
                entry.metadata.count += 1;
                if escalated {
                    entry.metadata.escalated_count += 1;
                }
                entry.metadata.min_timestamp = DateTime::from_nanos(timestamp as i64);
            } else {
                let alert = ApiAlertGroup {
                    id: rowid.to_string(),
                    source: source.clone(),
                    metadata: ApiAlertGroupMetadata {
                        count: 1,
                        escalated_count: if escalated { 1 } else { 0 },
                        min_timestamp: DateTime::from_nanos(timestamp as i64),
                        max_timestamp: DateTime::from_nanos(timestamp as i64),
                    },
                };
                events.insert(key.clone(), alert);
            }

            if count == 0 {
                info!("First row took {:?}", now.elapsed());

                // This kicks in the timer after the first result.
                now = Instant::now();
            }

            count += 1;

            if now.elapsed() > std::time::Duration::from_secs(3) {
                timed_out = true;
                break;
            }
        }

        info!(
            ?timed_out,
            "Alert query took {:?}, with {} events over {} groups",
            now.elapsed(),
            count,
            events.len()
        );

        let mut results: Vec<ApiAlertGroup> = vec![];
        for (_key, event) in events {
            results.push(event);
        }

        Ok(ApiAlertResponse {
            events: results,
            timed_out,
        })
    }

    #[instrument(skip_all)]
    pub async fn alerts(
        &self,
        options: AlertQueryOptions,
    ) -> Result<ApiAlertResponse, DatastoreError> {
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
             ORDER BY timestamp DESC"#;

        let mut from: Vec<&str> = Vec::new();
        let mut filters: Vec<String> = Vec::new();
        let mut args = SqliteArguments::default();

        from.push("events");

        filters.push("json_extract(events.source, '$.event_type') = 'alert'".to_string());

        for tag in options.tags {
            match tag.as_ref() {
                "evebox.archived" => {
                    filters.push("archived = ?".into());
                    args.add(1);
                }
                "-evebox.archived" => {
                    filters.push("archived = ?".into());
                    args.add(0);
                }
                "evebox.escalated" => {
                    filters.push("escalated = ?".into());
                    args.add(1);
                }
                _ => {}
            }
        }

        if let Some(sensor) = options.sensor {
            filters.push("json_extract(events.source, '$.host') = ?".into());
            args.add(sensor);
        }

        if let Some(ts) = options.timestamp_gte {
            filters.push("timestamp >= ?".into());
            args.add(ts.to_nanos());
        }

        // Query string.
        if let Some(query_string) = options.query_string {
            match queryparser::parse(&query_string, None) {
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
                                if el.negated {
                                    filters.push("events.source NOT LIKE ?".into());
                                    args.add(format!("%{s}%"));
                                } else {
                                    filters.push("events.source LIKE ?".into());
                                    args.add(format!("%{s}%"));
                                }
                            }
                            queryparser::QueryValue::KeyValue(k, v) => {
                                // TODO: Handle negation - maybe use query builder?
                                if let Ok(v) = v.parse::<i64>() {
                                    filters
                                        .push(format!("json_extract(events.source, '$.{k}') = ?"));
                                    args.add(v);
                                } else {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{k}') LIKE ?"
                                    ));
                                    args.add(format!("%{v}%"));
                                }
                            }
                            queryparser::QueryValue::From(_) => {
                                warn!("QueryValue::From not supported here");
                            }
                            queryparser::QueryValue::To(ts) => {
                                filters.push("timestamp <= ?".into());
                                args.add(ts.to_nanos());
                            }
                        }
                    }
                }
            }
        }

        let query = query.replace("%WHERE%", &filters.join(" AND "));
        let query = query.replace("%FROM%", &from.join(", "));

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &query, &args).await;
        } else if *LOG_QUERIES {
            info!("query={}; args={:?}", &query.trim(), &args);
        }

        let now = Instant::now();
        let mut rows = sqlx::query_with(&query, args).fetch(&self.pool);
        let mut results = vec![];
        while let Some(row) = rows.try_next().await? {
            results.push(alert_row_mapper(row)?);
        }

        debug!(
            "Rows={}, Elapsed={} ms",
            results.len(),
            now.elapsed().as_millis()
        );
        Ok(ApiAlertResponse {
            events: results,
            timed_out: false,
        })
    }
}

fn alert_row_mapper(row: SqliteRow) -> Result<ApiAlertGroup, DatastoreError> {
    let count: i64 = row.try_get(0)?;
    let id: i64 = row.try_get(1)?;
    let min_ts_nanos: i64 = row.try_get(2)?;

    let escalated_count: i64 = row.try_get(3)?;
    let archived: i8 = row.try_get(4)?;
    let mut parsed: serde_json::Value = row.try_get(5)?;

    if let serde_json::Value::Null = &parsed["tags"] {
        let tags: Vec<String> = Vec::new();
        parsed["tags"] = tags.into();
    }

    if let serde_json::Value::Array(ref mut tags) = &mut parsed["tags"] {
        if archived > 0 {
            tags.push("evebox.archived".into());
        }
    }

    let min_ts = DateTime::from_nanos(min_ts_nanos);
    let max_ts = crate::datetime::parse(&parsed["timestamp"].as_str().unwrap(), None)?;

    let alert = ApiAlertGroup {
        id: id.to_string(),
        source: parsed,
        metadata: ApiAlertGroupMetadata {
            count: count as u64,
            escalated_count: escalated_count as u64,
            min_timestamp: min_ts,
            max_timestamp: max_ts,
        },
    };

    Ok(alert)
}
