// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    eventrepo::DatastoreError,
    queryparser::QueryElement,
    sqlite::{builder::EventQueryBuilder, log_query_plan},
    LOG_QUERY_PLAN,
};
use futures::TryStreamExt;
use sqlx::Row;
use tracing::instrument;

use super::SqliteEventRepo;

impl SqliteEventRepo {
    #[instrument(skip_all)]
    pub async fn agg(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<QueryElement>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut builder = EventQueryBuilder::new(self.fts().await);
        builder
            .select(format!(
                "count(json_extract(events.source, '$.{field}')) as count"
            ))
            .select(format!("json_extract(events.source, '$.{field}')"))
            .from("events")
            .group_by(format!("json_extract(events.source, '$.{field}')"))
            .order_by("count", order)
            .limit(size as i64);

        // Some internal optimizing, may be provided on the query
        // string already.
        if field.starts_with("alert.") {
            builder.push_where("json_extract(events.source, '$.event_type') = 'alert'");
        } else if field.starts_with("dns.") {
            builder.push_where("json_extract(events.source, '$.event_type') = 'dns'");
        }

        builder.apply_query_string(&query);

        let mut results = vec![];
        let (sql, args) = builder.build();

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &args).await;
        }

        let mut rows = sqlx::query_with(&sql, args).fetch(&self.pool);
        while let Some(row) = rows.try_next().await? {
            let count: i64 = row.try_get(0)?;
            if count > 0 {
                // Rely on everything being a string in SQLite.
                let val: String = row.try_get(1)?;
                results.push(json!({"count": count, "key": val}));
            }
        }

        Ok(results)
    }
}
