// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::SQLiteEventStore;
use crate::datastore::DatastoreError;
use rusqlite::{types::Type, ToSql};
use std::fmt::Display;

impl SQLiteEventStore {
    pub async fn group_by(
        &self,
        field: &str,
        min_timestamp: time::OffsetDateTime,
        size: usize,
        order: &str,
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

        // TODO: Add an event_type field, or take it from the query
        // string. Really speeds things up.
        if field.starts_with("alert.") {
            builder.where_value("json_extract(events.source, '$.event_type') = ?", "alert");
        } else if field.starts_with("dns.") {
            builder.where_value("json_extract(events.source, '$.event_type') = ?", "dns");
        }

        let results = self
            .pool
            .get()
            .await?
            .interact(
                move |conn| -> Result<Vec<serde_json::Value>, DatastoreError> {
                    let tx = conn.transaction()?;
                    let mut st = tx.prepare(&builder.build())?;
                    let mut rows = st.query(rusqlite::params_from_iter(&builder.params))?;
                    let mut results = vec![];
                    while let Some(row) = rows.next()? {
                        let count: i64 = row.get(0)?;
                        if count > 0 {
                            let val: rusqlite::types::ValueRef = row.get_ref(1)?;
                            let val: serde_json::Value = match val.data_type() {
                                Type::Null => serde_json::Value::Null,
                                Type::Integer => val.as_i64()?.into(),
                                Type::Real => val.as_f64()?.into(),
                                Type::Text => val.as_str()?.into(),
                                Type::Blob => unimplemented!(),
                            };
                            results.push(json!([count, val]));
                        }
                    }
                    Ok(results)
                },
            )
            .await??;
        Ok(results)
    }
}

#[derive(Default)]
struct SelectQueryBuilder {
    select: Vec<String>,
    from: Vec<String>,
    wheres: Vec<String>,
    group_by: Vec<String>,
    order_by: Option<(String, String)>,
    limit: i64,
    params: Vec<Box<dyn ToSql + Send + Sync + 'static>>,
}

impl SelectQueryBuilder {
    fn new() -> Self {
        Default::default()
    }

    fn select<T: Into<String>>(&mut self, field: T) -> &mut Self {
        self.select.push(field.into());
        self
    }

    fn from<T: Into<String>>(&mut self, col: T) -> &mut Self {
        self.from.push(col.into());
        self
    }

    fn group_by<T: Into<String>>(&mut self, col: T) -> &mut Self {
        self.group_by.push(col.into());
        self
    }

    fn order_by<T: Into<String>>(&mut self, field: T, order: T) -> &mut Self {
        self.order_by = Some((field.into(), order.into()));
        self
    }

    /// Bind a parameter, basically set a where and a value.
    fn where_value<S, T>(&mut self, col: S, val: T) -> &mut Self
    where
        S: Into<String>,
        T: ToSql + Display + Send + Sync + 'static,
    {
        self.wheres.push(col.into());
        self.params.push(Box::new(val));
        self
    }

    /// Add a parameter.
    #[allow(dead_code)]
    fn param<T>(&mut self, v: T) -> &mut Self
    where
        T: ToSql + Display + Send + Sync + 'static,
    {
        self.params.push(Box::new(v));
        self
    }

    fn limit(&mut self, limit: i64) -> &mut Self {
        self.limit = limit;
        self
    }

    fn build(&mut self) -> String {
        let mut sql = String::new();

        sql.push_str("select ");
        sql.push_str(&self.select.join(", "));

        sql.push_str(" from ");
        sql.push_str(&self.from.join(", "));

        if !self.wheres.is_empty() {
            sql.push_str(" where ");
            sql.push_str(&self.wheres.join(" and "));
        }

        if !self.group_by.is_empty() {
            sql.push_str(" group by ");
            sql.push_str(&self.group_by.join(", "));
        }

        if let Some(order_by) = &self.order_by {
            sql.push_str(&format!(" order by {} {}", order_by.0, order_by.1));
        }

        if self.limit > 0 {
            sql.push_str(&format!(" limit {}", self.limit));
        }

        sql
    }
}
