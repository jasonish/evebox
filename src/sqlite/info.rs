// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

/// Sqlite database information wrapper.
///
/// Can take a transaction or connection.
pub(crate) struct Info<'a> {
    db: &'a mut sqlx::SqliteConnection,
}

impl<'a> Info<'a> {
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

    pub async fn pragma_i64(&mut self, name: &str) -> Result<i64, sqlx::Error> {
        let sql = format!("SELECT {name} FROM pragma_{name}");
        sqlx::query_scalar(&sql).fetch_one(&mut *self.db).await
    }

    pub async fn schema_version(&mut self) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&mut *self.db)
            .await
    }
}
