// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::SQLiteEventStore;
use crate::{
    datastore::DatastoreError,
    querystring::{self},
    sqlite::builder::SelectQueryBuilder,
};
use rusqlite::types::{FromSqlError, ValueRef};

impl SQLiteEventStore {
    pub async fn group_by(
        &self,
        field: &str,
        size: usize,
        order: &str,
        q: Vec<querystring::Element>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut builder = SelectQueryBuilder::new();
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
            builder.where_value("json_extract(events.source, '$.event_type') = ?", "alert");
        } else if field.starts_with("dns.") {
            builder.where_value("json_extract(events.source, '$.event_type') = ?", "dns");
        }

        builder.apply_query_string(&q);

        let results = self
            .pool
            .get()
            .await?
            .interact(
                move |conn| -> Result<Vec<serde_json::Value>, DatastoreError> {
                    let tx = conn.transaction()?;
                    let mut st = tx.prepare(&builder.sql())?;
                    let mut rows = st.query(rusqlite::params_from_iter(builder.params()))?;
                    let mut results = vec![];
                    while let Some(row) = rows.next()? {
                        let count: i64 = row.get(0)?;
                        if count > 0 {
                            let val = rusqlite_to_json(row.get_ref(1)?)?;
                            results.push(json!({"count": count, "key": val}));
                        }
                    }
                    Ok(results)
                },
            )
            .await??;
        Ok(results)
    }
}

fn rusqlite_to_json(val: ValueRef) -> Result<serde_json::Value, FromSqlError> {
    match val {
        ValueRef::Null => Ok(serde_json::Value::Null),
        ValueRef::Integer(_) => Ok(val.as_i64()?.into()),
        ValueRef::Real(_) => Ok(val.as_f64()?.into()),
        ValueRef::Text(_) => Ok(val.as_str()?.into()),
        // Not expected, at least not as of 2023-02-07.
        ValueRef::Blob(_) => unimplemented!(),
    }
}