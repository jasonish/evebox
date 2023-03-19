// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::querystring::{self, Element};
use rusqlite::{types::ToSqlOutput, ToSql};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) enum SqliteValue {
    String(String),
    I64(i64),
}

impl ToSql for SqliteValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self {
            Self::I64(v) => Ok(ToSqlOutput::Owned((*v).into())),
            Self::String(v) => Ok(ToSqlOutput::Owned(v.to_string().into())),
        }
    }
}

impl From<i64> for SqliteValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<String> for SqliteValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for SqliteValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

#[derive(Debug)]
pub(crate) struct NamedParams {
    params: HashMap<String, SqliteValue>,
}

impl NamedParams {
    pub fn new() -> Self {
        Self {
            params: HashMap::default(),
        }
    }

    pub fn set<K: Into<String>, V: Into<SqliteValue>>(&mut self, key: K, val: V) {
        self.params.insert(key.into(), val.into());
    }

    pub fn build(&self) -> Vec<(&str, &dyn ToSql)> {
        let mut params = vec![];
        for (k, v) in &self.params {
            let v: &dyn ToSql = v;
            params.push((&**k, v));
        }
        params
    }
}

#[derive(Default)]
pub(crate) struct EventQueryBuilder {
    /// Is FTS available?
    fts: bool,

    select: Vec<String>,
    from: Vec<String>,
    wheres: Vec<String>,
    group_by: Vec<String>,
    order_by: Option<(String, String)>,
    limit: i64,
    params: Vec<SqliteValue>,
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

    pub fn push_param<V: Into<SqliteValue>>(&mut self, val: V) -> &mut Self {
        self.params.push(val.into());
        self
    }

    pub fn limit(&mut self, limit: i64) -> &mut Self {
        self.limit = limit;
        self
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
                                _ => {
                                    if k.starts_with("dhcp") {
                                        self.push_fts(v);
                                    }
                                }
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
                    let fields = [
                        "src_ip",
                        "dest_ip",
                        "dhcp.assigned_ip",
                        "dhcp.client_ip",
                        "dhcp.next_server_ip",
                        "dhcp.routers",
                        "dhcp.relay_ip",
                        "dhcp.subnet_mask",
                    ];
                    let mut ors = vec![];
                    for field in fields {
                        ors.push(format!("json_extract(events.source, '$.{}') = ?", field));
                        self.push_param(ip.to_string());
                    }
                    self.push_where(format!("({})", ors.join(" OR ")));
                    if self.fts {
                        self.push_fts(ip);
                    }
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

    pub fn build(mut self) -> (String, Vec<SqliteValue>) {
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

        (sql, self.params)
    }
}
