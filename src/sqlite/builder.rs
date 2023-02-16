// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use rusqlite::ToSql;
use std::fmt::Display;

use crate::querystring::{self, Element};

#[derive(Default)]
pub struct EventQueryBuilder {
    /// Is FTS available?
    fts: bool,

    select: Vec<String>,
    from: Vec<String>,
    wheres: Vec<String>,
    group_by: Vec<String>,
    order_by: Option<(String, String)>,
    limit: i64,
    params: Vec<Box<dyn ToSql + Send + Sync + 'static>>,
    debug: Vec<String>,
    fts_phrases: Vec<String>,
}

impl EventQueryBuilder {
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

    /// Add a parameter.
    pub fn push_param<T>(&mut self, v: T) -> &mut Self
    where
        T: ToSql + Display + Send + Sync + 'static,
    {
        self.debug.push(v.to_string());
        self.params.push(Box::new(v));
        self
    }

    pub fn limit(&mut self, limit: i64) -> &mut Self {
        self.limit = limit;
        self
    }

    pub fn params(&self) -> &[Box<dyn ToSql + Send + Sync + 'static>] {
        &self.params
    }

    pub fn debug_params(&self) -> &[String] {
        &self.debug
    }

    pub fn apply_query_string(&mut self, q: &[querystring::Element]) {
        for e in q {
            match e {
                Element::String(s) => {
                    if self.fts {
                        self.push_fts(s);
                    } else {
                        self.push_where("events.source LIKE ?")
                            .push_param(format!("%{s}%"));
                    }
                }
                Element::KeyVal(k, v) => {
                    if let Ok(i) = v.parse::<i64>() {
                        self.push_where(format!("json_extract(events.source, '$.{k}') = ?"))
                            .push_param(i);
                    } else {
                        self.push_where(format!("json_extract(events.source, '$.{k}') = ?"))
                            .push_param(v.to_string());

                        // If FTS is enabled, some key/val searches
                        // can really benefit from it.
                        if self.fts {
                            match k.as_ref() {
                                "community_id" | "timestamp" => {
                                    self.push_fts(v);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Element::EarliestTimestamp(ts) => {
                    self.earliest_timestamp(ts);
                }
                Element::LatestTimestamp(ts) => {
                    self.latest_timestamp(ts);
                }
                Element::Ip(ip) => {
                    self.push_where("(json_extract(events.source, '$.src_ip') = ? OR json_extract(events.source, '$.dest_ip') = ?)")
			.push_param(ip.to_string())
			.push_param(ip.to_string());
                }
            }
        }
    }

    pub fn earliest_timestamp(&mut self, ts: &time::OffsetDateTime) -> &mut Self {
        self.push_where("timestamp >= ?")
            .push_param(ts.unix_timestamp_nanos() as i64);
        self
    }

    pub fn latest_timestamp(&mut self, ts: &time::OffsetDateTime) -> &mut Self {
        self.push_where("timestamp <= ?")
            .push_param(ts.unix_timestamp_nanos() as i64);
        self
    }

    pub fn build(
        mut self,
    ) -> (
        String,
        Vec<Box<dyn ToSql + Send + Sync + 'static>>,
        Vec<String>,
    ) {
        let mut sql = String::new();

        sql.push_str("select ");
        sql.push_str(&self.select.join(", "));

        sql.push_str(" from ");
        sql.push_str(&self.from.join(", "));

        if !self.fts_phrases.is_empty() {
            let query = self.fts_phrases.join(" AND ");
            self.push_where("events.rowid in (select rowid from fts where fts match ?)");
            self.push_param(query);
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

        (sql, self.params, self.debug)
    }
}
