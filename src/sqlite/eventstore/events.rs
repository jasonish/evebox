// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::{
    datastore::{DatastoreError, EventQueryParams},
    eve::eve::EveJson,
    querystring::Element,
};

use super::{ParamBuilder, SQLiteEventStore};

impl SQLiteEventStore {
    pub async fn events(
        &self,
        options: EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        let result = self
            .pool
            .get()
            .await?
            .interact(
                move |conn| -> Result<Vec<serde_json::Value>, rusqlite::Error> {
                    let query = r#"
		    SELECT 
			events.rowid AS id, 
			events.archived AS archived, 
			events.escalated AS escalated, 
			events.source AS source
		    FROM %FROM%
		    WHERE %WHERE%
		    ORDER BY events.timestamp %ORDER%
		    LIMIT 500
		"#;
                    let mut from: Vec<&str> = vec![];
                    let mut filters: Vec<String> = vec![];
                    let mut params = ParamBuilder::new();

                    from.push("events");

                    if let Some(event_type) = options.event_type {
                        filters.push("json_extract(events.source, '$.event_type') = ?".to_string());
                        params.push(event_type);
                    }

                    if let Some(dt) = options.max_timestamp {
                        filters.push("timestamp <= ?".to_string());
                        params.push(dt.unix_timestamp_nanos() as i64);
                    }

                    if let Some(dt) = options.min_timestamp {
                        filters.push("timestamp >= ?".to_string());
                        params.push(dt.unix_timestamp_nanos() as i64);
                    }
                    for element in &options.query_string_elements {
                        match element {
                            Element::String(val) => {
                                filters.push("events.source LIKE ?".into());
                                params.push(format!("%{val}%"));
                            }
                            Element::KeyVal(key, val) => {
                                if let Ok(val) = val.parse::<i64>() {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{key}') = ?"
                                    ));
                                    params.push(val);
                                } else {
                                    filters.push(format!(
                                        "json_extract(events.source, '$.{key}') LIKE ?"
                                    ));
                                    params.push(format!("%{val}%"));
                                }
                            }
                            Element::BeforeTimestamp(_) => todo!(),
                            Element::AfterTimestamp(_) => todo!(),
                            Element::Ip(_) => todo!(),
                            Element::EarliestTimestamp(_) => todo!(),
                            Element::LatestTimestamp(_) => todo!(),
                        }
                    }

                    let order = if let Some(order) = options.order {
                        order
                    } else {
                        "DESC".to_string()
                    };

                    let query = query.replace("%FROM%", &from.join(", "));
                    let query = query.replace("%WHERE%", &filters.join(" AND "));
                    let query = query.replace("%ORDER%", &order);

                    // TODO: Cleanup query building.
                    let mut query = query;
                    if filters.is_empty() {
                        query = query.replace("WHERE", "");
                    }

                    let tx = conn.transaction()?;
                    let mut st = tx.prepare(&query)?;
                    let rows =
                        st.query_and_then(rusqlite::params_from_iter(&params.params), row_mapper)?;
                    let mut events = vec![];
                    for row in rows {
                        events.push(row?);
                    }
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
