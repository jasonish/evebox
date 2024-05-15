// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use futures::TryStreamExt;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::Arguments;
use sqlx::Row;
use tracing::{debug, error, info};

use super::SqliteEventRepo;
use crate::LOG_QUERIES;
use crate::{
    elastic::AlertQueryOptions,
    eventrepo::DatastoreError,
    querystring::{self, Element},
    sqlite::format_sqlite_timestamp,
};
use std::time::Instant;

impl SqliteEventRepo {
    pub async fn alerts(
        &self,
        options: AlertQueryOptions,
    ) -> Result<serde_json::Value, DatastoreError> {
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
            args.add(ts.unix_timestamp_nanos() as i64);
        }

        // Query string.
        if let Some(query_string) = options.query_string {
            match querystring::parse(&query_string, None) {
                Err(err) => {
                    error!(
                        "Failed to parse query string: error={}, query string={}",
                        &err, &query_string
                    );
                }
                Ok(elements) => {
                    for el in &elements {
                        debug!("Parsed query string element: {:?}", el);
                        match el {
                            Element::String(val) => {
                                filters.push("events.source LIKE ?".into());
                                args.add(format!("%{val}%"));
                            }
                            Element::NotString(val) => {
                                filters.push("events.source NOT LIKE ?".into());
                                args.add(format!("%{val}%"));
                            }
                            Element::KeyVal(key, val) => {
                                if let Ok(val) = val.parse::<i64>() {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{key}') = ?"
                                    ));
                                    args.add(val);
                                } else {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{key}') LIKE ?"
                                    ));
                                    args.add(format!("%{val}%"));
                                }
                            }
                            Element::Ip(_) => todo!(),
                            Element::EarliestTimestamp(_) => todo!(),
                            Element::LatestTimestamp(_) => todo!(),
                        }
                    }
                }
            }
        }

        let query = query.replace("%WHERE%", &filters.join(" AND "));
        let query = query.replace("%FROM%", &from.join(", "));

        if *LOG_QUERIES {
            info!("query={}", &query.trim());
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
        let response = json!({
            "events": results,
        });
        Ok(response)
    }
}

fn alert_row_mapper(row: SqliteRow) -> Result<serde_json::Value, DatastoreError> {
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
            tags.push("archived".into());
            tags.push("evebox.archived".into());
        }
    }

    let min_ts = time::OffsetDateTime::from_unix_timestamp_nanos(min_ts_nanos as i128)?
        .to_offset(time::UtcOffset::UTC);

    let alert = json!({
        "_id": id,
        "_source": parsed,
        "_metadata": json!({
            "count": count,
            "escalated_count": escalated_count,
            "min_timestamp": format_sqlite_timestamp(&min_ts),
            "max_timestamp": &parsed["timestamp"],
            "aggregate": true,
        }),
    });

    Ok(alert)
}
