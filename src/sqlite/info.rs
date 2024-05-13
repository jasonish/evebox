// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use rusqlite::{params, types::FromSql, OptionalExtension};

/// Sqlite database information wrapper.
///
/// Can take a transaction or connection.
pub(crate) struct Infox<'a> {
    db: &'a mut sqlx::SqliteConnection,
}

impl<'a> Infox<'a> {
    pub fn new(db: &'a mut sqlx::SqliteConnection) -> Self {
        Self { db }
    }

    pub async fn get_auto_vacuum(&mut self) -> Result<u8, sqlx::Error> {
        sqlx::query_scalar("SELECT auto_vacuum FROM pragma_auto_vacuum")
            .fetch_one(&mut *self.db)
            .await
    }

    pub async fn get_journal_mode(&mut self) -> Result<String, sqlx::Error> {
        sqlx::query_scalar("SELECT journal_mode FROM pragma_journal_mode")
            .fetch_one(&mut *self.db)
            .await
    }

    pub async fn get_synchronous(&mut self) -> Result<u8, sqlx::Error> {
        sqlx::query_scalar("SELECT synchronous FROM pragma_synchronous")
            .fetch_one(&mut *self.db)
            .await
    }

    pub async fn has_table(&mut self, name: &str) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(name)
        .fetch_one(&mut *self.db)
        .await?;
        Ok(count > 0)
    }
}

pub(crate) struct Info<'a> {
    conn: &'a rusqlite::Connection,
}

impl<'a> Info<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn get_auto_vacuum(&self) -> Result<u8, rusqlite::Error> {
        self.conn
            .query_row_and_then("SELECT auto_vacuum FROM pragma_auto_vacuum", [], |row| {
                row.get(0)
            })
    }

    pub fn get_synchronous(&self) -> Result<u8, rusqlite::Error> {
        self.conn
            .query_row_and_then("SELECT synchronous FROM pragma_synchronous", [], |row| {
                row.get(0)
            })
    }

    pub fn has_table(&self, name: &str) -> Result<bool, rusqlite::Error> {
        let row = self
            .conn
            .query_row(
                "select name from sqlite_master where type = 'table' and name = ?",
                params![name],
                |_| Ok(()),
            )
            .optional()?;
        Ok(row.is_some())
    }

    pub fn get_pragma<T: FromSql>(&self, name: &str) -> Result<T, rusqlite::Error> {
        let sql = format!("SELECT {name} FROM pragma_{name}");
        self.conn.query_row_and_then(&sql, [], |row| row.get(0))
    }

    pub fn schema_version(&self) -> Result<u64, rusqlite::Error> {
        let schema_version: u64 = self.conn.query_row_and_then(
            "select max(version) from refinery_schema_history",
            [],
            |row| row.get(0),
        )?;
        Ok(schema_version)
    }
}
