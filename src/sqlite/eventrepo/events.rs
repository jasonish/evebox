// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::SqliteEventRepo;
use crate::{
    eventrepo::{DatastoreError, EventQueryParams},
    sqlite::{builder::EventQueryBuilder, log_query_plan},
    LOG_QUERIES, LOG_QUERY_PLAN,
};
use futures::TryStreamExt;
use sqlx::{sqlite::SqliteRow, Row};
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

impl SqliteEventRepo {
    #[instrument(skip_all)]
    pub async fn events(
        &self,
        options: EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        let mut builder = EventQueryBuilder::new(self.fts().await);
        builder
            .select("events.rowid AS id")
            .select("events.archived AS archived")
            .select("events.escalated AS escalated")
            .select("events.source AS source");
        builder.from("events");
        builder.limit(500);

        if let Some(event_type) = options.event_type {
            builder
                .push_where("json_extract(events.source, '$.event_type') = ?")
                .push_arg(event_type);
        }

        if let Some(dt) = &options.max_timestamp {
            warn!("Found deprecated parameter 'max_timestamp' in SQLite events query");
            builder.latest_timestamp(dt);
        }

        if let Some(dt) = &options.min_timestamp {
            warn!("Found deprecated parameter 'min_timestamp' in SQLite events query");
            builder.earliest_timestamp(dt);
        }

        builder.apply_query_string(&options.query_string);

        if let Some(order) = &options.order {
            builder.order_by("events.timestamp", order);
        } else {
            builder.order_by("events.timestamp", "DESC");
        }

        let (sql, params) = builder.build();

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &params).await;
        } else if *LOG_QUERIES {
            info!("query={}; args={:?}", &sql.trim(), &params);
        }

        let now = Instant::now();
        let mut rows = sqlx::query_with(&sql, params).fetch(&self.pool);
        let mut events = vec![];
        while let Some(row) = rows.try_next().await? {
            events.push(row_mapper(row)?);
        }

        debug!(
            "Rows={}, Elapsed={} ms",
            events.len(),
            now.elapsed().as_millis()
        );

        let response = json!({
            "ecs": false,
            "events": events,
        });
        Ok(response)
    }
}

fn row_mapper(row: SqliteRow) -> Result<serde_json::Value, sqlx::Error> {
    let id: i64 = row.try_get(0)?;
    let archived: i8 = row.try_get(1)?;
    let escalated: i8 = row.try_get(2)?;
    let mut parsed: serde_json::Value = row.try_get(3)?;

    if let Some(timestamp) = parsed.get("timestamp") {
        parsed["@timestamp"] = timestamp.clone();
    }

    if let serde_json::Value::Null = &parsed["tags"] {
        let tags: Vec<String> = Vec::new();
        parsed["tags"] = tags.into();
    }

    if let serde_json::Value::Array(ref mut tags) = &mut parsed["tags"] {
        if archived > 0 {
            tags.push("evebox.archived".into());
        }
        if escalated > 0 {
            tags.push("evebox.escalated".into());
        }
    }

    let event = json!({
        "_id": id,
        "_source": parsed,
    });
    Ok(event)
}
