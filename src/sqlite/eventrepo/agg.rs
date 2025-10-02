// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crate::datetime::DateTime;
use crate::prelude::*;
use crate::sqlite::prelude::*;
use crate::{
    LOG_QUERY_PLAN,
    queryparser::QueryElement,
    sqlite::{builder::EventQueryBuilder, log_query_plan},
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::instrument;

use super::SqliteEventRepo;

fn to_sorted_vec<'a>(results: &'a HashMap<String, i64>, order: &str) -> Vec<(&'a String, &'a i64)> {
    let mut sorted: Vec<_> = results.iter().collect();
    if order == "asc" {
        sorted.sort_by(|a, b| a.1.cmp(b.1));
    } else {
        sorted.sort_by(|a, b| b.1.cmp(a.1));
    }
    sorted
}

impl SqliteEventRepo {
    pub(crate) async fn agg_stream(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<QueryElement>,
        tx: Option<UnboundedSender<serde_json::Value>>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut builder = EventQueryBuilder::new(self.fts().await);
        builder.select("timestamp");

        if field == "dns.rrname" {
            let coa =
                "coalesce(source->>'dns'->>'queries'->>0->>'rrname', source->>'dns'->>'rrname')";
            builder.select(format!("{coa} as agg"));
        } else {
            // Always cast to TEXT to handle both strings and integers consistently
            builder.select(format!(
                "CAST(json_extract(events.source, '$.{field}') AS TEXT) as agg"
            ));
        }

        builder.push_where("agg IS NOT NULL");

        builder.from("events");
        builder.order_by("timestamp", "desc");

        builder.apply_query_string(&query)?;

        // Some internal optimizing, may be provided on the query
        // string already.
        if field.starts_with("alert.") {
            builder.push_where("json_extract(events.source, '$.event_type') = 'alert'");
        } else if field.starts_with("dns.") {
            builder.push_where("json_extract(events.source, '$.event_type') = 'dns'");
        }

        let (sql, args) = builder.build()?;

        let mut results: HashMap<String, i64> = HashMap::new();

        let mut rows = sqlx::query_with(&sql, args).fetch(&self.pool);
        let mut now = std::time::Instant::now();
        let mut timestamp: i64 = 0;
        while let Some(row) = rows.try_next().await? {
            timestamp = row.try_get(0)?;
            let agg: String = row.try_get(1)?;
            let entry = results.entry(agg.clone()).or_insert(0);
            *entry += 1;

            if now.elapsed() >= std::time::Duration::from_secs(1) {
                let rows: Vec<serde_json::Value> = to_sorted_vec(&results, order)
                    .iter()
                    .take(size)
                    .map(|(k, v)| json!({"key": k, "count": v}))
                    .collect();

                let response = json!({
                    "rows": rows,
                    "done": false,
                    "earliest_ts": DateTime::from_nanos(timestamp),
                });

                if let Some(tx) = &tx {
                    if let Err(err) = tx.send(response) {
                        debug!(
                            "Failed to send agg update to channel, aborting: error={:?}",
                            err
                        );
                        return Ok(vec![]);
                    }
                }

                now = std::time::Instant::now();
            }
        }

        let rows: Vec<serde_json::Value> = to_sorted_vec(&results, order)
            .iter()
            .take(size)
            .map(|(k, v)| json!({"key": k, "count": v}))
            .collect();
        let response = json!({
            "rows": rows.clone(),
            "done": true,
            "earliest_ts": DateTime::from_nanos(timestamp),
        });

        if let Some(tx) = &tx {
            if let Err(err) = tx.send(response.clone()) {
                error!(
                    "Failed to send agg update to channel, aborting: error={:?}",
                    err
                );
            }
        }

        Ok(rows)
    }

    #[instrument(skip_all)]
    pub(crate) async fn agg(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<QueryElement>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut builder = EventQueryBuilder::new(self.fts().await);

        if field == "dns.rrname" {
            let coa =
                "coalesce(source->>'dns'->>'queries'->>0->>'rrname', source->>'dns'->>'rrname')";
            builder.select(format!("count({coa}) as count"));
            builder.select(format!("{coa} as agg"));
        } else {
            builder.select(format!(
                "count(json_extract(events.source, '$.{field}')) as count"
            ));
            // Always cast to TEXT to handle both strings and integers consistently
            builder.select(format!(
                "CAST(json_extract(events.source, '$.{field}') AS TEXT) as agg"
            ));
        }
        builder.from("events");
        builder.group_by("agg");
        builder.order_by("count", order);
        builder.limit(size as i64);

        // Some internal optimizing, may be provided on the query
        // string already.
        if field.starts_with("alert.") {
            builder.push_where("json_extract(events.source, '$.event_type') = 'alert'");
        } else if field.starts_with("dns.") {
            builder.push_where("json_extract(events.source, '$.event_type') = 'dns'");
        }

        builder.apply_query_string(&query)?;

        let mut results = vec![];
        let (sql, args) = builder.build()?;

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &args).await;
        }

        let mut rows = sqlx::query_with(&sql, args).fetch(&self.pool);
        while let Some(row) = rows.try_next().await? {
            let count: i64 = row.try_get(0)?;
            if count > 0 {
                let val: String = row.try_get(1)?;
                results.push(json!({"count": count, "key": val}));
            }
        }

        Ok(results)
    }

    pub(crate) async fn get_event_types(&self, query: Vec<QueryElement>) -> Result<Vec<String>> {
        let mut builder = EventQueryBuilder::new(self.fts().await);
        builder.select("distinct json_extract(events.source, '$.event_type')");
        builder.from("events");
        builder.apply_query_string(&query)?;

        let (sql, args) = builder.build()?;

        let rows: Vec<String> = sqlx::query_scalar_with(&sql, args)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }
}
