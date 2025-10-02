// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use crate::sqlite::prelude::*;

use crate::{
    LOG_QUERIES, LOG_QUERY_PLAN,
    datetime::DateTime,
    eventrepo::StatsAggQueryParams,
    queryparser::{QueryElement, QueryValue},
    sqlite::{builder::EventQueryBuilder, log_query_plan, log_query_plan2},
    util,
};
use futures::TryStreamExt;
use serde::Serialize;
use std::time::Instant;
use tracing::{debug, info};

use super::SqliteEventRepo;

impl SqliteEventRepo {
    pub(crate) async fn histogram_time(
        &self,
        interval: Option<u64>,
        query: &[QueryElement],
    ) -> Result<Vec<serde_json::Value>> {
        // The timestamp (in seconds) of the latest event to
        // consider. This is to determine the bucket interval as well
        // as fill wholes at the end of the dataset.
        let now = DateTime::now().to_seconds();

        let mut conn = self.pool.acquire().await?;

        let from = query
            .iter()
            .find(|e| matches!(e.value, QueryValue::From(_)))
            .map(|e| match e.value {
                QueryValue::From(ref v) => v,
                _ => unreachable!(),
            });
        let earliest = if let Some(from) = from {
            from.clone()
        } else if let Some(earliest) = Self::get_earliest_timestamp(&mut conn).await? {
            crate::datetime::DateTime::from_nanos(earliest)
        } else {
            return Ok(vec![]);
        };

        let interval = if let Some(interval) = interval {
            interval
        } else {
            let interval = util::histogram_interval(now - earliest.to_seconds());
            debug!("No interval provided by client, using {interval}s");
            interval
        };

        let last_time = now / (interval as i64) * (interval as i64);
        let mut next_time = ((earliest.to_seconds() as u64) / interval * interval) as i64;

        let timestamp = format!("timestamp / 1000000000 / {interval} * {interval}");

        let mut builder = EventQueryBuilder::new(self.fts().await);

        builder.select(&timestamp);
        builder.select(format!("count({timestamp})"));
        builder.from("events");
        builder.group_by(timestamp.to_string());
        builder.order_by("timestamp", "asc");

        builder.apply_query_string(query)?;

        let (sql, params) = builder.build()?;

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &params).await;
        }

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
            let debug = DateTime::from_seconds(time);

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

        while next_time <= last_time {
            let dt = DateTime::from_seconds(next_time);
            results.push(Element {
                time: next_time * 1000,
                count: 0,
                debug: dt.to_eve(),
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
        let sql = "SELECT MIN(timestamp) FROM events";
        if *LOG_QUERY_PLAN {
            log_query_plan2(&mut *conn, sql, &SqliteArguments::default()).await;
        }
        sqlx::query_scalar(sql).fetch_optional(&mut *conn).await
    }

    async fn get_stats(&self, qp: &StatsAggQueryParams) -> Result<Vec<(i64, i64)>> {
        let qp = qp.clone();
        let field = format!("$.{}", &qp.field);
        let start_time = qp.start_time.to_nanos();
        let end_time = qp.end_time.to_nanos();
        let range = (qp.end_time.datetime - qp.start_time.datetime).num_seconds();
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
        args.push(&field)?;

        let mut filters = vec![
            "json_extract(events.source, '$.event_type') = 'stats'",
            "timestamp >= ?",
            "timestamp <= ?",
        ];
        args.push(start_time)?;
        args.push(end_time)?;

        if let Some(sensor_name) = qp.sensor_name.as_ref() {
            if sensor_name == "(no-name)" {
                filters.push("json_extract(events.source, '$.host') IS NULL");
            } else {
                filters.push("json_extract(events.source, '$.host') = ?");
                args.push(sensor_name)?;
            }
        }

        let sql = sql.replace("%WHERE%", &filters.join(" AND "));
        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &args).await;
        }

        if *LOG_QUERIES {
            info!("sql={}, params={:?}", &sql, &args);
        }

        let timer = Instant::now();

        let rows: Vec<(i64, i64)> = sqlx::query_as_with(&sql, args)
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
                    "timestamp": DateTime::from_seconds(*timestamp).to_rfc3339_utc(),
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
    ) -> anyhow::Result<serde_json::Value> {
        let field = format!("$.{}", &params.field);
        let start_time = params.start_time.to_nanos();
        let end_time = params.end_time.to_nanos();
        let range = (params.end_time.datetime - params.start_time.datetime).num_seconds();
        let interval = crate::util::histogram_interval(range);

        let mut args = SqliteArguments::default();

        // Get sensor data without COALESCE for better performance
        let sql = format!(
            "
            SELECT
              json_extract(events.source, '$.host') AS sensor,
              (timestamp / 1000000000 / {interval}) * {interval} AS bucket_time,
              MAX(json_extract(events.source, ?))
            FROM events
            WHERE json_extract(events.source, '$.event_type') = 'stats'
              AND timestamp >= ?
              AND timestamp <= ?
            GROUP BY sensor, bucket_time
            ORDER BY sensor, bucket_time
            "
        );
        args.push(&field)?;
        args.push(start_time)?;
        args.push(end_time)?;

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &args).await;
        }

        if *LOG_QUERIES {
            info!("sql={}, params={:?}", &sql, &args);
        }

        let timer = Instant::now();

        let rows: Vec<(Option<String>, i64, Option<i64>)> = sqlx::query_as_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        debug!(
            "Returning {} stats records by sensor in {} ms",
            rows.len(),
            timer.elapsed().as_millis()
        );

        // Group data by sensor in the exact format Elasticsearch returns
        let mut sensor_data: std::collections::HashMap<String, Vec<serde_json::Value>> =
            std::collections::HashMap::new();

        for (sensor, timestamp, value) in rows {
            // Map NULL sensor to "(no-name)" in Rust for better performance
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
    ) -> anyhow::Result<serde_json::Value> {
        let field = format!("$.{}", &params.field);
        let start_time = params.start_time.to_nanos();
        let end_time = params.end_time.to_nanos();
        let range = (params.end_time.datetime - params.start_time.datetime).num_seconds();
        let interval = crate::util::histogram_interval(range);

        let mut args = SqliteArguments::default();

        // Get sensor data without COALESCE for better performance
        let sql = format!(
            "
            SELECT
              json_extract(events.source, '$.host') AS sensor,
              (timestamp / 1000000000 / {interval}) * {interval} AS bucket_time,
              MAX(json_extract(events.source, ?))
            FROM events
            WHERE json_extract(events.source, '$.event_type') = 'stats'
              AND timestamp >= ?
              AND timestamp <= ?
            GROUP BY sensor, bucket_time
            ORDER BY sensor, bucket_time
            "
        );
        args.push(&field)?;
        args.push(start_time)?;
        args.push(end_time)?;

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &args).await;
        }

        if *LOG_QUERIES {
            info!("sql={}, params={:?}", &sql, &args);
        }

        let timer = Instant::now();

        let rows: Vec<(Option<String>, i64, Option<i64>)> = sqlx::query_as_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        debug!(
            "Returning {} stats diff records by sensor in {} ms",
            rows.len(),
            timer.elapsed().as_millis()
        );

        // Group data by sensor and calculate differentials
        let mut sensor_data: std::collections::HashMap<String, Vec<serde_json::Value>> =
            std::collections::HashMap::new();
        let mut previous_values: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();

        for (sensor, timestamp, value) in rows {
            // Map NULL sensor to "(no-name)" in Rust for better performance
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
            // Always update the previous value for this sensor
            previous_values.insert(sensor_name, value);
        }

        Ok(json!({
            "data": sensor_data,
        }))
    }
}
