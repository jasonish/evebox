// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::queryparser;
use sqlx::sqlite::SqliteArguments;
use sqlx::Arguments;

#[derive(Default)]
pub(crate) struct EventQueryBuilder<'a> {
    /// Is FTS available?
    fts: bool,

    select: Vec<String>,
    from: Vec<String>,
    wheres: Vec<String>,
    group_by: Vec<String>,
    order_by: Option<(String, String)>,
    limit: i64,
    fts_phrases: Vec<String>,

    args: SqliteArguments<'a>,
}

impl<'a> EventQueryBuilder<'a> {
    pub fn new(fts: bool) -> Self {
        Self {
            fts,
            ..Default::default()
        }
    }

    pub fn select<T: Into<String>>(&mut self, field: T) -> &mut Self {
        self.select.push(field.into());
        self
    }

    pub fn from<T: Into<String>>(&mut self, col: T) -> &mut Self {
        self.from.push(col.into());
        self
    }

    pub fn group_by<T: Into<String>>(&mut self, col: T) -> &mut Self {
        self.group_by.push(col.into());
        self
    }

    pub fn order_by<T: Into<String>>(&mut self, field: T, order: T) -> &mut Self {
        self.order_by = Some((field.into(), order.into()));
        self
    }

    /// Add a where statement without requiring a value.
    pub fn push_where<S>(&mut self, col: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.wheres.push(col.into());
        self
    }

    pub fn push_fts<S>(&mut self, val: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.fts_phrases.push(format!("\"{}\"", val.into()));
        self
    }

    pub fn push_arg<T>(&mut self, value: T)
    where
        T: sqlx::Encode<'a, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite> + 'a,
    {
        self.args.add(value)
    }

    pub fn limit(&mut self, limit: i64) -> &mut Self {
        self.limit = limit;
        self
    }

    pub fn where_source_json(&mut self, field: &str, op: &str, value: &str) -> &mut Self {
        self.push_where(format!("json_extract(events.source, '$.{field}') {op} ?"));
        if let Ok(i) = value.parse::<i64>() {
            self.push_arg(i);
        } else {
            self.push_arg(value.to_string());
        }
        self
    }

    pub fn apply_query_string(&mut self, q: &[queryparser::QueryElement]) {
        for e in q {
            match &e.value {
                queryparser::QueryValue::String(s) => {
                    if e.negated {
                        self.push_where("events.source NOT LIKE ?")
                            .push_arg(format!("%{s}%"));
                    } else if self.fts {
                        self.push_fts(s);
                    } else {
                        self.push_where("events.source LIKE ?")
                            .push_arg(format!("%{s}%"));
                    }
                }
                queryparser::QueryValue::KeyValue(k, v) => {
                    match k.as_ref() {
                        "@ip" | "@mac" => {
                            if e.negated {
                                self.push_where("events.source NOT LIKE ?")
                                    .push_arg(format!("%{v}%"));
                            } else if self.fts {
                                self.push_fts(v);
                            } else {
                                self.push_where("events.source LIKE ?")
                                    .push_arg(format!("%{v}%"));
                            }
                        }
                        _ => {
                            if e.negated {
                                self.where_source_json(k, "!=", v);
                            } else {
                                self.where_source_json(k, "=", v);
                                // If FTS is enabled, some key/val searches
                                // can really benefit from it.
                                if self.fts {
                                    match k.as_ref() {
                                        "community_id" | "timestamp" => {
                                            self.push_fts(v);
                                        }
                                        _ => {
                                            if k.starts_with("dhcp") {
                                                self.push_fts(v);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                queryparser::QueryValue::From(ts) => {
                    self.timestamp_gte(ts);
                }
                queryparser::QueryValue::To(ts) => {
                    self.timestamp_lte(ts);
                }
            }
        }
    }

    pub fn earliest_timestamp(&mut self, ts: &DateTime) -> &mut Self {
        self.push_where("timestamp >= ?").push_arg(ts.to_nanos());
        self
    }

    pub fn timestamp_gte(&mut self, ts: &crate::datetime::DateTime) -> &mut Self {
        self.push_where("timestamp >= ?").push_arg(ts.to_nanos());
        self
    }

    pub fn timestamp_lte(&mut self, ts: &crate::datetime::DateTime) -> &mut Self {
        self.push_where("timestamp <= ?").push_arg(ts.to_nanos());
        self
    }

    pub fn latest_timestamp(&mut self, ts: &DateTime) -> &mut Self {
        self.push_where("timestamp <= ?").push_arg(ts.to_nanos());
        self
    }

    pub fn build(&mut self) -> (String, SqliteArguments<'a>) {
        let mut sql = String::new();

        sql.push_str("select ");
        sql.push_str(&self.select.join(", "));

        sql.push_str(" from ");
        sql.push_str(&self.from.join(", "));

        if !self.fts_phrases.is_empty() {
            let query = self.fts_phrases.join(" AND ");
            self.push_where("events.rowid in (select rowid from fts where fts match ?)");
            self.push_arg(query);
        }

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

        (sql, self.args.clone())
    }
}
