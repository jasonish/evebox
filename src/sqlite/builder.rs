// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use std::fmt::Display;
use rusqlite::ToSql;

#[derive(Default)]
pub struct SelectQueryBuilder {
    select: Vec<String>,
    from: Vec<String>,
    wheres: Vec<String>,
    group_by: Vec<String>,
    order_by: Option<(String, String)>,
    limit: i64,
    params: Vec<Box<dyn ToSql + Send + Sync + 'static>>,
}

impl SelectQueryBuilder {
    pub fn new() -> Self {
        Default::default()
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

    /// Bind a parameter, basically set a where and a value.
    pub fn where_value<S, T>(&mut self, col: S, val: T) -> &mut Self
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
    pub fn param<T>(&mut self, v: T) -> &mut Self
    where
        T: ToSql + Display + Send + Sync + 'static,
    {
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

    pub fn build(&mut self) -> String {
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
