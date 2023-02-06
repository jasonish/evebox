// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::{QueryParam, SQLiteEventStore};
use crate::prelude::*;
use crate::{
    datastore::DatastoreError,
    elastic::AlertQueryOptions,
    searchquery::{self, Element},
    sqlite::{eventstore::parse_timestamp, format_sqlite_timestamp},
};

impl SQLiteEventStore {
    pub async fn alerts(
        &self,
        options: AlertQueryOptions,
    ) -> Result<serde_json::Value, DatastoreError> {
        let conn = self.pool.get().await?;
        let result = conn
            .interact(
                move |conn| -> Result<Vec<serde_json::Value>, DatastoreError> {
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
                        match tag.as_ref() {
                            "evebox.archived" => {
                                filters.push("archived = ?".into());
                                params.push(Box::new(1));
                            }
                            "-evebox.archived" => {
                                filters.push("archived = ?".into());
                                params.push(Box::new(0));
                            }
                            "evebox.escalated" => {
                                filters.push("escalated = ?".into());
                                params.push(Box::new(1));
                            }
                            _ => {}
                        }
                    }

                    if let Some(ts) = options.timestamp_gte {
                        filters.push("timestamp >= ?".into());
                        params.push(Box::new(ts.unix_timestamp_nanos() as i64));
                    }

                    // Query string.
                    if let Some(query_string) = options.query_string {
                        match searchquery::parse(&query_string) {
                            Err(err) => {
                                error!(
                                    "Failed to parse query string: error={}, query string={}",
                                    &err, &query_string
                                );
                            }
                            Ok((unparsed, elements)) => {
                                if !unparsed.is_empty() {
                                    error!("Unparsed characters in query string: {unparsed}");
                                }
                                for el in &elements {
                                    debug!("Parsed query string element: {:?}", el);
                                    match el {
                                        Element::String(val) => {
                                            filters.push("events.source LIKE ?".into());
                                            params.push(Box::new(format!("%{val}%")));
                                        }
                                        Element::KeyVal(key, val) => match key.as_ref() {
                                            "@before" => {
                                                if let Ok(ts) = parse_timestamp(val) {
                                                    filters.push("timestamp <= ?".to_string());
                                                    params.push(Box::new(
                                                        ts.unix_timestamp_nanos() as i64,
                                                    ));
                                                } else {
                                                    error!(
                                                        "Failed to parse {} timestamp of {}",
                                                        &key, &val
                                                    );
                                                }
                                            }
                                            "@after" => {
                                                if let Ok(ts) = parse_timestamp(val) {
                                                    filters.push("timestamp >= ?".to_string());
                                                    params.push(Box::new(
                                                        ts.unix_timestamp_nanos() as i64,
                                                    ));
                                                } else {
                                                    error!(
                                                        "Failed to parse {} timestamp of {}",
                                                        &key, &val
                                                    );
                                                }
                                            }
                                            _ => {
                                                if let Ok(val) = val.parse::<i64>() {
                                                    filters.push(format!(
                                                    "json_extract(events.source, '$.{key}') = ?"
                                                ));
                                                    params.push(Box::new(val));
                                                } else {
                                                    filters.push(format!(
                                                    "json_extract(events.source, '$.{key}') LIKE ?"
                                                ));
                                                    params.push(Box::new(format!("%{val}%")));
                                                }
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }

                    let query = query.replace("%WHERE%", &filters.join(" AND "));
                    let query = query.replace("%FROM%", &from.join(", "));

                    let tx = conn.transaction()?;
                    let mut st = tx.prepare(&query)?;
                    let rows =
                        st.query_and_then(rusqlite::params_from_iter(params), alert_row_mapper)?;
                    let mut results = vec![];
                    for row in rows {
                        results.push(row?);
                    }
                    Ok(results)
                },
            )
            .await??;
        let response = json!({
            "events": result,
        });
        Ok(response)
    }
}

fn alert_row_mapper(row: &rusqlite::Row) -> Result<serde_json::Value, DatastoreError> {
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
