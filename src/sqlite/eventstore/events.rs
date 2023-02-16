// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::SQLiteEventStore;
use crate::{
    datastore::{DatastoreError, EventQueryParams},
    eve::eve::EveJson,
    sqlite::builder::EventQueryBuilder,
    LOG_QUERIES,
};
use std::time::Instant;
use tracing::{debug, info};

impl SQLiteEventStore {
    pub async fn events(
        &self,
        options: EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        let mut builder = EventQueryBuilder::new(self.fts);
        let result = self
            .pool
            .get()
            .await?
            .interact(
                move |conn| -> Result<Vec<serde_json::Value>, rusqlite::Error> {
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
                            .push_param(event_type);
                    }

                    if let Some(dt) = &options.max_timestamp {
                        builder.latest_timestamp(dt);
                    }

                    if let Some(dt) = &options.min_timestamp {
                        builder.earliest_timestamp(dt);
                    }

                    builder.apply_query_string(&options.query_string_elements);

                    if let Some(order) = &options.order {
                        builder.order_by("events.timestamp", order);
                    } else {
                        builder.order_by("events.timestamp", "DESC");
                    }

                    let (sql, params, debug_params) = builder.build();

                    if *LOG_QUERIES {
                        info!("query={} args={:?}", &sql, debug_params);
                    }

                    let tx = conn.transaction()?;
                    let mut st = tx.prepare(&sql)?;
                    let now = Instant::now();
                    let rows = st.query_and_then(rusqlite::params_from_iter(params), row_mapper)?;
                    let mut events = vec![];
                    for row in rows {
                        events.push(row?);
                    }
                    debug!(
                        "Rows={}, Elapsed={} ms",
                        events.len(),
                        now.elapsed().as_millis()
                    );
                    Ok(events)
                },
            )
            .await??;
        let response = json!({
            "ecs": false,
            "events": result,
        });
        Ok(response)
    }
}

fn row_mapper(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    let id: i64 = row.get(0)?;
    let archived: i8 = row.get(1)?;
    let escalated: i8 = row.get(2)?;
    let mut parsed: EveJson = row.get(3)?;

    if let Some(timestamp) = parsed.get("timestamp") {
        parsed["@timestamp"] = timestamp.clone();
    }

    if let serde_json::Value::Null = &parsed["tags"] {
        let tags: Vec<String> = Vec::new();
        parsed["tags"] = tags.into();
    }

    if let serde_json::Value::Array(ref mut tags) = &mut parsed["tags"] {
        if archived > 0 {
            tags.push("archived".into());
            tags.push("evebox.archived".into());
        }
        if escalated > 0 {
            tags.push("escalated".into());
            tags.push("evebox.escalated".into());
        }
    }

    let event = json!({
        "_id": id,
        "_source": parsed,
    });
    Ok(event)
}
