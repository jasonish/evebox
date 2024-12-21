// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::queryparser;
use crate::sqlite::prelude::*;

#[derive(Default)]
pub(crate) struct EventQueryBuilder<'a> {
    /// Is FTS available?
    fts: bool,

    select: Vec<String>,
    from: Vec<String>,
    left_join: Vec<String>,
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

    pub fn selectjs<T: Into<String>>(&mut self, field: T) -> &mut Self {
        let field: String = field.into();
        self.select.push(format!(
            "json_extract(events.source, '$.{field}') AS '{field}'"
        ));
        self
    }

    pub fn selectjs2<T: Into<String>>(&mut self, field: T) -> &mut Self {
        let field: String = field.into();
        self.select
            .push(format!("events.source->>'{field}' AS '{field}'"));
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

    pub fn wherejs<F, O, A>(&mut self, field: F, op: O, arg: A) -> Result<&mut Self, sqlx::Error>
    where
        F: Into<String>,
        O: Into<String>,
        A: sqlx::Encode<'a, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite> + 'a,
    {
        let field: String = field.into();
        let op: String = op.into();
        self.wheres
            .push(format!("json_extract(events.source, '$.{field}') {op} ?"));
        self.push_arg(arg)?;
        Ok(self)
    }

    pub fn push_fts<S>(&mut self, val: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.fts_phrases.push(format!("\"{}\"", val.into()));
        self
    }

    pub fn push_arg<T>(&mut self, value: T) -> Result<(), sqlx::Error>
    where
        T: sqlx::Encode<'a, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite> + 'a,
    {
        self.args.push(value)
    }

    pub fn limit(&mut self, limit: i64) -> &mut Self {
        self.limit = limit;
        self
    }

    /// Create a `where` expression using `json_extract`.
    ///
    /// This is the older way of extracting JSON from before the ->>
    /// operator, but it needs to be used if json_extract was used in
    /// the indexes.
    fn where_source_json_extract(
        &mut self,
        field: &str,
        op: &str,
        value: &str,
    ) -> Result<&mut Self, sqlx::Error> {
        self.push_where(format!("json_extract(events.source, '$.{field}') {op} ?"));
        if let Ok(i) = value.parse::<i64>() {
            self.push_arg(i)?;
        } else {
            self.push_arg(value.to_string())?;
        }
        Ok(self)
    }

    /// Create a `where` expression using `->>` where the value is
    /// extracted as a JSON value (real, integer, etc).
    fn where_source_json(
        &mut self,
        field: &str,
        op: &str,
        value: &str,
    ) -> Result<&mut Self, sqlx::Error> {
        self.push_where(format!("events.source->>'{field}' {op} ?"));
        if let Ok(i) = value.parse::<i64>() {
            self.push_arg(i)?;
        } else {
            self.push_arg(value.to_string())?;
        }
        Ok(self)
    }

    pub fn add_left_join(&mut self, sql: String) {
        if !self.left_join.contains(&sql) {
            self.left_join.push(sql);
        }
    }

    pub fn left_join_from_query_string(
        &mut self,
        q: &'a [queryparser::QueryElement],
    ) -> Result<(), sqlx::Error> {
        for e in q {
            match &e.value {
                queryparser::QueryValue::KeyValue(k, _v) => {
                    //if k == "dns.rrname" || k == "dns.queries.rrname" {
                    if k == "dns.rrname" || k.starts_with("dns.queries") {
                        self.add_left_join(
                            "LEFT JOIN json_each(events.source, '$.dns.queries') AS _dns_queries"
                                .to_string(),
                        );
                    } else if k.starts_with("dns.answers.") {
                        self.add_left_join(
                            "LEFT JOIN json_each(events.source, '$.dns.answers') AS _dns_answers"
                                .to_string(),
                        );
                    }
                }
                queryparser::QueryValue::String(_) => {}
                queryparser::QueryValue::From(_) => {}
                queryparser::QueryValue::To(_) => {}
            }
        }
        Ok(())
    }

    pub fn apply_query_string(
        &mut self,
        q: &'a [queryparser::QueryElement],
    ) -> Result<(), sqlx::Error> {
        for e in q {
            match &e.value {
                queryparser::QueryValue::String(s) => {
                    if e.negated {
                        self.push_where("events.source NOT LIKE ?")
                            .push_arg(format!("%{s}%"))?;
                    } else if self.fts {
                        self.push_fts(s);
                    } else {
                        self.push_where("events.source LIKE ?")
                            .push_arg(format!("%{s}%"))?;
                    }
                }
                queryparser::QueryValue::KeyValue(k, v) => {
                    match k.as_ref() {
                        "@ip" | "@mac" => {
                            if e.negated {
                                self.push_where("events.source NOT LIKE ?")
                                    .push_arg(format!("%{v}%"))?;
                            } else if self.fts {
                                self.push_fts(v);
                            } else {
                                self.push_where("events.source LIKE ?")
                                    .push_arg(format!("%{v}%"))?;
                            }
                        }
                        // These fields use '->>' style JSON extraction.
                        "src_port" | "dest_port" => {
                            if e.negated {
                                self.where_source_json(k, "!=", v)?;
                            } else {
                                self.where_source_json(k, "=", v)?;
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
                        _ => {
                            if e.negated {
                                self.where_source_json_extract(k, "!=", v)?;
                            } else {
                                if k == "dns.type" && (v == "query" || v == "request") {
                                    self.push_where("(events.source->>'dns'->>'type' = 'query' OR events.source->>'dns'->>'type' = 'request')");
                                } else if k == "dns.type" && (v == "response" || v == "answer") {
                                    self.push_where("(events.source->>'dns'->>'type' = 'answer' OR events.source->>'dns'->>'type' = 'response')");
                                } else if k == "dns.rrname" || k == "dns.queries.rrname" {
                                    self.push_where("(events.source->>'dns'->>'rrname' = ? OR _dns_queries.value->>'rrname' = ?)");
                                    self.push_arg(v)?;
                                    self.push_arg(v)?;
                                } else if k.starts_with("dns.queries.") {
                                    let path: String = k
                                        .split('.')
                                        .skip(2)
                                        .map(|p| format!("'{}'", p))
                                        .collect::<Vec<String>>()
                                        .join("->>");
                                    self.push_where(format!("_dns_queries.value->>{} = ?", path));
                                    self.push_arg(v)?;
                                } else if k.starts_with("dns.answers.") {
                                    let path: String = k
                                        .split('.')
                                        .skip(2)
                                        .map(|p| format!("'{}'", p))
                                        .collect::<Vec<String>>()
                                        .join("->>");
                                    self.push_where(format!("_dns_answers.value->>{} = ?", path));
                                    self.push_arg(v)?;
                                } else if k.starts_with("dns.authorities") {
                                    // Lazy helper - can't be done with Elastic though.
                                    self.push_where("events.source->>'dns'->>'authorities' GLOB ?");
                                    self.push_arg(format!("*{}*", v))?;
                                } else if k.starts_with("dns.additionals") {
                                    // Lazy helper - can't be done with Elastic though.
                                    self.push_where("events.source->>'dns'->>'additionals' GLOB ?");
                                    self.push_arg(format!("*{}*", v))?;
                                } else {
                                    self.where_source_json_extract(k, "=", v)?;
                                }
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
                    self.timestamp_gte(ts)?;
                }
                queryparser::QueryValue::To(ts) => {
                    self.timestamp_lte(ts)?;
                }
            }
        }
        Ok(())
    }

    pub fn earliest_timestamp(&mut self, ts: &DateTime) -> Result<&mut Self, sqlx::Error> {
        self.push_where("timestamp >= ?").push_arg(ts.to_nanos())?;
        Ok(self)
    }

    pub fn timestamp_gte(
        &mut self,
        ts: &crate::datetime::DateTime,
    ) -> Result<&mut Self, sqlx::Error> {
        self.push_where("timestamp >= ?").push_arg(ts.to_nanos())?;
        Ok(self)
    }

    pub fn timestamp_lte(
        &mut self,
        ts: &crate::datetime::DateTime,
    ) -> Result<&mut Self, sqlx::Error> {
        self.push_where("timestamp <= ?").push_arg(ts.to_nanos())?;
        Ok(self)
    }

    pub fn latest_timestamp(&mut self, ts: &DateTime) -> Result<&mut Self, sqlx::Error> {
        self.push_where("timestamp <= ?").push_arg(ts.to_nanos())?;
        Ok(self)
    }

    pub fn build(&mut self) -> Result<(String, SqliteArguments<'a>), sqlx::Error> {
        let mut sql = String::new();

        sql.push_str("select ");
        sql.push_str(&self.select.join(", "));

        sql.push_str(" from ");
        sql.push_str(&self.from.join(", "));

        for left_join in &self.left_join {
            sql.push_str(&format!(" {}", left_join));
        }

        if !self.fts_phrases.is_empty() {
            let query = self.fts_phrases.join(" AND ");
            self.push_where("events.rowid in (select rowid from fts where fts match ?)");
            self.push_arg(query)?;
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

        Ok((sql, self.args.clone()))
    }
}
