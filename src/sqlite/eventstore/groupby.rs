// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::SQLiteEventStore;
use crate::{
    datastore::DatastoreError, querystring::Element, server::api::QueryStringParts,
    sqlite::builder::SelectQueryBuilder,
};
use rusqlite::types::{FromSqlError, ValueRef};

impl SQLiteEventStore {
    pub async fn group_by(
        &self,
        field: &str,
        min_timestamp: time::OffsetDateTime,
        size: usize,
        order: &str,
        q: Option<QueryStringParts>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut builder = SelectQueryBuilder::new();
        builder
            .select(format!(
                "count(json_extract(events.source, '$.{field}')) as count"
            ))
            .select(format!("json_extract(events.source, '$.{field}')"))
            .from("events")
            .where_value(
                "timestamp >= ?",
                min_timestamp.unix_timestamp_nanos() as u64,
            )
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

        if let Some(q) = &q {
            for e in &q.elements {
                match e {
                    Element::String(s) => {
                        builder.where_value("events.source LIKE ?", s.clone());
                    }
                    Element::KeyVal(k, v) => match k.as_ref() {
                        "@ip" => {
                            dbg!((k, v));
                            builder.push_where("(json_extract(events.source, '$.src_ip') = ? OR json_extract(events.source, '$.dest_ip') = ?)");
                            builder.push_param(v.clone());
                            builder.push_param(v.clone());
                        }
                        _ => {
                            builder.where_value(
                                format!("json_extract(events.source, '$.{k}') = ?"),
                                v.clone(),
                            );
                        }
                    },
                    Element::BeforeTimestamp(_) => todo!(),
                    Element::AfterTimestamp(_) => todo!(),
                    Element::Ip(_) => todo!(),
                }
            }
        }

        let results = self
            .pool
            .get()
            .await?
            .interact(
                move |conn| -> Result<Vec<serde_json::Value>, DatastoreError> {
                    let tx = conn.transaction()?;
                    let mut st = tx.prepare(&builder.build())?;
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
