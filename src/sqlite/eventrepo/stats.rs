// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::SqliteEventRepo;
use crate::{
    eventrepo::{DatastoreError, StatsAggQueryParams},
    queryparser::{QueryElement, QueryValue},
    sqlite::{builder::EventQueryBuilder, eventrepo::nanos_to_rfc3339, format_sqlite_timestamp},
    util, LOG_QUERIES,
};
use futures::TryStreamExt;
use serde::Serialize;
use sqlx::sqlite::SqliteArguments;
use sqlx::Arguments;
use sqlx::Row;
use sqlx::SqliteConnection;
use std::time::Instant;
use tracing::{debug, info};

impl SqliteEventRepo {
    pub(crate) async fn histogram_time(
        &self,
        interval: Option<u64>,
        query: &[QueryElement],
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        // The timestamp (in seconds) of the latest event to
        // consider. This is to determine the bucket interval as well
        // as fill wholes at the end of the dataset.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();

        let mut conn = self.pool.acquire().await?;

        let from = query
            .iter()
            .find(|e| matches!(e.value, QueryValue::From(_)))
            .map(|e| match e.value {
                QueryValue::From(ref v) => v,
                _ => unreachable!(),
            });
        let earliest = if let Some(from) = from {
            *from
        } else if let Some(earliest) = Self::get_earliest_timestamp(&mut conn).await? {
            chrono::DateTime::from_timestamp_nanos(earliest)
        } else {
            return Ok(vec![]);
        };

        let interval = if let Some(interval) = interval {
            interval
        } else {
            let interval = util::histogram_interval(now - earliest.timestamp());
            debug!("No interval provided by client, using {interval}s");
            interval
        };

        let last_time = now / (interval as i64) * (interval as i64);
        let mut next_time = ((earliest.timestamp() as u64) / interval * interval) as i64;

        let timestamp = format!("timestamp / 1000000000 / {interval} * {interval}");

        let mut builder = EventQueryBuilder::new(self.fts);

        builder.select(&timestamp);
        builder.select(format!("count({timestamp})"));
        builder.from("events");
        builder.group_by(timestamp.to_string());
        builder.order_by("timestamp", "asc");

        builder.apply_new_query_string(query);

        let (sql, params) = builder.build();

        if *LOG_QUERIES {
            info!("sql={sql}, params={:?}", &params);
        }

        let timer = Instant::now();

        #[derive(Debug, Serialize)]
        struct Element {
            time: i64,
            count: u64,
            debug: String,
        }

        let mut results = vec![];
        let mut stream = sqlx::query_with(&sql, params).fetch(&mut *conn);
        while let Some(row) = stream.try_next().await? {
            let time: i64 = row.try_get(0)?;
            let count: i64 = row.try_get(1)?;
            let debug = time::OffsetDateTime::from_unix_timestamp(time).unwrap();

            while next_time < time {
                let dt = time::OffsetDateTime::from_unix_timestamp(next_time).unwrap();
                results.push(Element {
                    time: next_time * 1000,
                    count: 0,
                    debug: format_sqlite_timestamp(&dt),
                });
                next_time += interval as i64;
            }
            results.push(Element {
                time: time * 1000,
                count: count as u64,
                debug: format_sqlite_timestamp(&debug),
            });
            next_time += interval as i64;
        }

        while next_time <= last_time {
            let dt = time::OffsetDateTime::from_unix_timestamp(next_time).unwrap();
            results.push(Element {
                time: next_time * 1000,
                count: 0,
                debug: format_sqlite_timestamp(&dt),
            });
            next_time += interval as i64;
        }

        if *LOG_QUERIES {
            info!(
                "Query time: {} ms: rows={}",
                timer.elapsed().as_millis(),
                results.len()
            );
        }

        let response: Vec<serde_json::Value> = results
            .iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect();
        Ok(response)
    }

    async fn get_earliest_timestamp(
        conn: &mut SqliteConnection,
    ) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar("SELECT MIN(timestamp) FROM events")
            .fetch_optional(&mut *conn)
            .await
    }

    async fn get_stats(&self, qp: &StatsAggQueryParams) -> anyhow::Result<Vec<(u64, u64)>> {
        let qp = qp.clone();
        let field = format!("$.{}", &qp.field);
        let start_time = qp.start_time.unix_timestamp_nanos() as i64;
        let range = (time::OffsetDateTime::now_utc() - qp.start_time).whole_seconds();
        let interval = crate::util::histogram_interval(range);

        let mut args = SqliteArguments::default();

        let sql = format!(
            "
            SELECT
              (timestamp / 1000000000 / {interval}) * {interval} AS a,
              MAX(json_extract(events.source, ?))
              FROM events
              WHERE %WHERE%
              GROUP BY a
              ORDER BY a
            "
        );
        args.add(&field);

        let mut filters = vec![
            "json_extract(events.source, '$.event_type') = 'stats'",
            "timestamp >= ?",
        ];
        args.add(start_time);

        if let Some(sensor_name) = qp.sensor_name.as_ref() {
            filters.push("json_extract(events.source, '$.host') = ?");
            args.add(sensor_name);
        }

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));
        if *LOG_QUERIES {
            info!("sql={}, params={:?}", &sql, &args);
        }

        let timer = Instant::now();

        let rows: Vec<(u64, u64)> = sqlx::query_as_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        debug!(
            "Returning {} stats records for {field} in {} ms",
            rows.len(),
            timer.elapsed().as_millis()
        );
        Ok(rows)
    }

    pub async fn stats_agg(
        &self,
        params: &StatsAggQueryParams,
    ) -> anyhow::Result<serde_json::Value> {
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
        Ok(json!({
            "data": response_data,
        }))
    }

    pub async fn stats_agg_diff(
        &self,
        params: &StatsAggQueryParams,
    ) -> anyhow::Result<serde_json::Value> {
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
        Ok(json!({
            "data": response_data,
        }))
    }
}
