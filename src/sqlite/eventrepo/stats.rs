// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::SqliteEventRepo;
use crate::prelude::*;
use crate::{
    eventrepo::{DatastoreError, StatsAggQueryParams},
    querystring::{self, QueryString},
    sqlite::{builder::EventQueryBuilder, eventrepo::nanos_to_rfc3339, format_sqlite_timestamp},
    util, LOG_QUERIES,
};
use rusqlite::{Connection, OptionalExtension};

impl SqliteEventRepo {
    pub(crate) async fn histogram_time(
        &self,
        interval: Option<u64>,
        q: &[querystring::Element],
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut builder = EventQueryBuilder::new(self.fts);
        let q = q.to_vec();
        self.pool
            .get()
            .await?
            .interact(move |conn| -> Result<_, _> {
                // The timestamp (in seconds) of the latest event to
                // consider. This is to determine the bucket interval
                // as well as fill wholes at the end of the dataset.
                let now = time::OffsetDateTime::now_utc().unix_timestamp();

                // Get the earliest timestamp, either from the query or the database.
                let earliest = if let Some(earliest) = q.get_earliest() {
                    earliest
                } else if let Some(earliest) = Self::get_earliest_timestamp(conn)? {
                    let earliest =
                        time::OffsetDateTime::from_unix_timestamp_nanos(earliest as i128)?;
                    debug!(
                        "No time-range provided by client, using earliest from database of {}",
                        &earliest
                    );
                    earliest
                } else {
                    return Ok(vec![]);
                };

                let interval = if let Some(interval) = interval {
                    interval
                } else {
                    let interval = util::histogram_interval(now - earliest.unix_timestamp());
                    debug!("No interval provided by client, using {interval}s");
                    interval
                };

                let last_time = now / (interval as i64) * (interval as i64);
                let mut next_time =
                    ((earliest.unix_timestamp() as u64) / interval * interval) as i64;

                let timestamp = format!("timestamp / 1000000000 / {interval} * {interval}");
                builder.select(&timestamp);
                builder.select(format!("count({timestamp})"));
                builder.from("events");
                builder.group_by(timestamp.to_string());
                builder.order_by("timestamp", "asc");

                builder.apply_query_string(&q);

                let (sql, params, _) = builder.build();
                let mut st = conn.prepare(&sql)?;
                let mut rows = st.query(rusqlite::params_from_iter(params))?;
                let mut results = vec![];
                while let Some(row) = rows.next()? {
                    let time: i64 = row.get(0)?;
                    let count: i64 = row.get(1)?;
                    let debug = time::OffsetDateTime::from_unix_timestamp(time).unwrap();
                    while next_time < time {
                        let dt = time::OffsetDateTime::from_unix_timestamp(next_time).unwrap();
                        results.push(json!({"time": next_time * 1000,
					    "count": 0,
					    "debug": format_sqlite_timestamp(&dt)}));
                        next_time += interval as i64;
                    }
                    results.push(json!({"time": time * 1000,
			       "count": count,
			       "debug": format_sqlite_timestamp(&debug)}));
                    next_time += interval as i64;
                }
                while next_time <= last_time {
                    let dt = time::OffsetDateTime::from_unix_timestamp(next_time).unwrap();
                    results.push(json!({"time": next_time * 1000,
					"count": 0,
					"debug": format_sqlite_timestamp(&dt)}));
                    next_time += interval as i64;
                }
                Ok(results)
            })
            .await?
    }

    fn get_earliest_timestamp(conn: &Connection) -> Result<Option<i64>, rusqlite::Error> {
        conn.query_row("select min(timestamp) from events", [], |row| {
            let timestamp: i64 = row.get(0)?;
            Ok(timestamp)
        })
        .optional()
    }

    async fn get_stats(&self, qp: &StatsAggQueryParams) -> Result<Vec<(u64, u64)>> {
        let qp = qp.clone();
        let conn = self.pool.get().await?;
        let field = format!("$.{}", &qp.field);
        let start_time = qp.start_time.unix_timestamp_nanos() as i64;
        let range = (time::OffsetDateTime::now_utc() - qp.start_time).whole_seconds();
        let interval = crate::util::histogram_interval(range);
        let result = conn
            .interact(move |conn| -> Result<Vec<(u64, u64)>, rusqlite::Error> {
                let sql = r#"
                        SELECT
                            (timestamp / 1000000000 / :interval) * :interval AS a,
                            MAX(json_extract(events.source, :field))
                        FROM events
                        WHERE %WHERE%
                        GROUP BY a
                        ORDER BY a
                    "#;

                let mut filters = vec![
                    "json_extract(events.source, '$.event_type') = 'stats'",
                    "timestamp >= :start_time",
                ];
                let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = vec![
                    (":interval", &interval),
                    (":field", &field),
                    (":start_time", &start_time),
                ];
                if let Some(sensor_name) = qp.sensor_name.as_ref() {
                    filters.push("+json_extract(events.source, '$.host') = :sensor_name");
                    params.push((":sensor_name", sensor_name));
                }
                let sql = sql.replace("%WHERE%", &filters.join(" AND "));
                if *LOG_QUERIES {
                    info!(
                        "sql={}, interval={interval}, field={field}, start_time={start_time}",
                        &sql
                    );
                }
                let mut stmt = conn.prepare(&sql)?;
                let rows =
                    stmt.query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?;
                let mut entries = vec![];
                for row in rows {
                    entries.push(row?);
                }
                Ok(entries)
            })
            .await
            .map_err(|err| anyhow!("sqlite interact error:: {:?}", err))??;
        Ok(result)
    }

    pub async fn stats_agg(&self, params: &StatsAggQueryParams) -> Result<serde_json::Value> {
        let rows = self.get_stats(params).await?;
        let response_data: Vec<serde_json::Value> = rows
            .iter()
            .map(|(timestamp, value)| {
                json!({
                    "value": value,
                    "timestamp": nanos_to_rfc3339((timestamp * 1000000000) as i128).unwrap(),
                })
            })
            .collect();
        return Ok(json!({
            "data": response_data,
        }));
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
                "timestamp": nanos_to_rfc3339((e.0 * 1000000000) as i128)?,
            }));
        }
        return Ok(json!({
            "data": response_data,
        }));
    }
}
