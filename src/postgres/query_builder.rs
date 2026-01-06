// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::queryparser;
use sqlx::Arguments;
use sqlx::postgres::PgArguments;

#[derive(Default)]
pub(crate) struct EventQueryBuilder {
    select: Vec<String>,
    from: Vec<String>,
    wheres: Vec<String>,
    order_by: Option<(String, String)>,
    limit: i64,
    args: PgArguments,
    arg_count: usize,
}

impl EventQueryBuilder {
    pub(crate) fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub(crate) fn select<T: Into<String>>(&mut self, field: T) -> &mut Self {
        self.select.push(field.into());
        self
    }

    pub(crate) fn from<T: Into<String>>(&mut self, col: T) -> &mut Self {
        self.from.push(col.into());
        self
    }

    pub(crate) fn order_by<T: Into<String>>(&mut self, field: T, order: T) -> &mut Self {
        self.order_by = Some((field.into(), order.into()));
        self
    }

    pub(crate) fn next_placeholder(&mut self) -> String {
        self.arg_count += 1;
        format!("${}", self.arg_count)
    }

    pub(crate) fn push_where<S>(&mut self, col: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.wheres.push(col.into());
        self
    }

    pub(crate) fn push_arg<T>(&mut self, value: T) -> Result<(), sqlx::Error>
    where
        T: 'static
            + sqlx::Encode<'static, sqlx::Postgres>
            + sqlx::Type<sqlx::Postgres>
            + Send
            + Sync,
    {
        self.args.add(value).map_err(sqlx::Error::Encode)?;
        Ok(())
    }

    pub(crate) fn limit(&mut self, limit: i64) -> &mut Self {
        self.limit = limit;
        self
    }

    pub(crate) fn timestamp_gte(
        &mut self,
        ts: &crate::datetime::DateTime,
    ) -> Result<&mut Self, sqlx::Error> {
        let p = self.next_placeholder();
        self.push_where(format!("timestamp >= {}", p))
            .push_arg(ts.datetime.to_utc())?;
        Ok(self)
    }

    pub(crate) fn timestamp_gt(
        &mut self,
        ts: &crate::datetime::DateTime,
    ) -> Result<&mut Self, sqlx::Error> {
        let p = self.next_placeholder();
        self.push_where(format!("timestamp > {}", p))
            .push_arg(ts.datetime.to_utc())?;
        Ok(self)
    }

    pub(crate) fn timestamp_lte(
        &mut self,
        ts: &crate::datetime::DateTime,
    ) -> Result<&mut Self, sqlx::Error> {
        let p = self.next_placeholder();
        self.push_where(format!("timestamp <= {}", p))
            .push_arg(ts.datetime.to_utc())?;
        Ok(self)
    }

    pub(crate) fn timestamp_lt(
        &mut self,
        ts: &crate::datetime::DateTime,
    ) -> Result<&mut Self, sqlx::Error> {
        let p = self.next_placeholder();
        self.push_where(format!("timestamp < {}", p))
            .push_arg(ts.datetime.to_utc())?;
        Ok(self)
    }

    pub(crate) fn where_source_json(
        &mut self,
        field: &str,
        op: &str,
        value: &str,
    ) -> Result<&mut Self, sqlx::Error> {
        let p = self.next_placeholder();
        // Use ->> for text extraction
        self.push_where(format!("source->>'{}' {} {}", field, op, p));
        if let Ok(i) = value.parse::<i64>() {
            self.push_arg(i)?;
        } else {
            self.push_arg(value.to_string())?;
        }
        Ok(self)
    }

    pub(crate) fn apply_query_string(
        &mut self,
        q: &[queryparser::QueryElement],
    ) -> Result<(), sqlx::Error> {
        for e in q {
            match &e.value {
                queryparser::QueryValue::String(s) => {
                    let p = self.next_placeholder();
                    if e.negated {
                        self.push_where(format!(
                            "NOT source_values_tsv @@ plainto_tsquery('simple', {})",
                            p
                        ))
                        .push_arg(s.to_string())?;
                    } else {
                        self.push_where(format!(
                            "source_values_tsv @@ plainto_tsquery('simple', {})",
                            p
                        ))
                        .push_arg(s.to_string())?;
                    }
                }
                queryparser::QueryValue::KeyValue(k, v) => {
                    let k = match k.as_ref() {
                        "@sid" => "alert.signature_id",
                        "@sig" => "alert.signature",
                        "@ip" | "@mac" => {
                            let p = self.next_placeholder();
                            if e.negated {
                                self.push_where(format!(
                                    "NOT source_values_tsv @@ plainto_tsquery('simple', {})",
                                    p
                                ))
                                .push_arg(v.to_string())?;
                            } else {
                                self.push_where(format!(
                                    "source_values_tsv @@ plainto_tsquery('simple', {})",
                                    p
                                ))
                                .push_arg(v.to_string())?;
                            }
                            continue;
                        }
                        _ => k,
                    };

                    if e.negated {
                        self.where_source_json(k, "!=", v)?;
                    } else {
                        // Simplify JSON path handling for now
                        if k.contains('.') {
                            // "a.b.c" -> "source->'a'->'b'->>'c'"
                            let parts: Vec<&str> = k.split('.').collect();
                            let mut path = "source".to_string();
                            for (i, part) in parts.iter().enumerate() {
                                if i == parts.len() - 1 {
                                    path.push_str(&format!("->>'{}'", part));
                                } else {
                                    path.push_str(&format!("->'{}'", part));
                                }
                            }
                            let p = self.next_placeholder();
                            self.push_where(format!("{} = {}", path, p));

                            if let Ok(i) = v.parse::<i64>() {
                                self.push_arg(i)?;
                            } else {
                                self.push_arg(v.to_string())?;
                            }
                        } else {
                            self.where_source_json(k, "=", v)?;
                        }
                    }
                }
                queryparser::QueryValue::From(ts) => {
                    self.timestamp_gte(ts)?;
                }
                queryparser::QueryValue::To(ts) => {
                    self.timestamp_lte(ts)?;
                }
                queryparser::QueryValue::After(ts) => {
                    self.timestamp_gt(ts)?;
                }
                queryparser::QueryValue::Before(ts) => {
                    self.timestamp_lt(ts)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn build(&mut self) -> Result<(String, PgArguments), sqlx::Error> {
        let mut sql = String::new();

        sql.push_str("SELECT ");
        sql.push_str(&self.select.join(", "));

        sql.push_str(" FROM ");
        sql.push_str(&self.from.join(", "));

        if !self.wheres.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&self.wheres.join(" AND "));
        }

        if let Some(order_by) = &self.order_by {
            sql.push_str(&format!(" ORDER BY {} {}", order_by.0, order_by.1));
        }

        if self.limit > 0 {
            sql.push_str(&format!(" LIMIT {}", self.limit));
        }

        Ok((sql, std::mem::take(&mut self.args)))
    }
}
