// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub mod builder;
pub mod configdb;
pub mod connection;
pub mod eventrepo;
pub mod importer;
pub(crate) mod info;
pub mod retention;
pub mod util;

pub(crate) use connection::ConnectionBuilder;
use sqlx::Arguments;
use sqlx::{SqliteConnection, SqliteExecutor, SqlitePool, sqlite::SqliteArguments};
use tracing::{error, instrument};

pub(crate) async fn has_table<'a>(
    conn: impl SqliteExecutor<'a>,
    name: &str,
) -> Result<bool, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(name)
            .fetch_one(conn)
            .await?;
    Ok(count > 0)
}

#[instrument(skip_all)]
async fn log_query_plan<'a>(pool: &SqlitePool, sql: &str, args: &SqliteArguments<'a>) {
    let rows: Result<Vec<(i64, i64, i64, String)>, sqlx::Error> =
        sqlx::query_as_with(&format!("explain query plan {}", &sql), args.clone())
            .fetch_all(pool)
            .await;
    match rows {
        Err(err) => {
            error!("Failed to explain query plan: {}: sql={}", err, sql);
        }
        Ok(rows) => {
            tracing::info!(?args, "{}", sql.replace("\n", ""));
            for row in rows {
                tracing::info!("{}", row.3);
            }
        }
    }
}

#[instrument(skip_all)]
async fn log_query_plan2<'a>(pool: &mut SqliteConnection, sql: &str, args: &SqliteArguments<'a>) {
    let rows: Result<Vec<(i64, i64, i64, String)>, sqlx::Error> =
        sqlx::query_as_with(&format!("explain query plan {}", &sql), args.clone())
            .fetch_all(pool)
            .await;
    match rows {
        Err(err) => {
            error!("Failed to explain query plan: {}: sql={}", err, sql);
        }
        Ok(rows) => {
            tracing::info!(?args, "{sql}");
            for row in rows {
                tracing::info!("{}", row.3);
            }
        }
    }
}

pub(crate) trait SqliteArgumentsExt<'a> {
    fn push<T>(&mut self, value: T) -> Result<(), sqlx::error::Error>
    where
        T: sqlx::Encode<'a, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite> + 'a;
}

impl<'a> SqliteArgumentsExt<'a> for SqliteArguments<'a> {
    fn push<T>(&mut self, value: T) -> Result<(), sqlx::error::Error>
    where
        T: sqlx::Encode<'a, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite> + 'a,
    {
        self.add(value).map_err(sqlx::error::Error::Encode)
    }
}

#[allow(dead_code)]
pub(crate) trait EveBoxSqlxErrorExt {
    fn is_interrupted(&self) -> bool;
    fn is_locked(&self) -> bool;
}

impl EveBoxSqlxErrorExt for sqlx::error::Error {
    fn is_interrupted(&self) -> bool {
        if let Some(err) = self.as_database_error() {
            if err.message() == "interrupted" {
                return true;
            }
        }
        false
    }

    fn is_locked(&self) -> bool {
        if let Some(err) = self.as_database_error() {
            if err.message() == "database is locked" {
                return true;
            }
        }
        false
    }
}

#[allow(unused_imports)]
pub(crate) mod prelude {
    pub use sqlx::Arguments;
    pub use sqlx::Connection;
    pub use sqlx::FromRow;
    pub use sqlx::Row;
    pub use sqlx::SqliteConnection;
    pub use sqlx::SqlitePool;
    pub use sqlx::sqlite::SqliteArguments;
    pub use sqlx::sqlite::SqliteRow;

    pub use futures::TryStreamExt;

    pub(crate) use super::SqliteArgumentsExt;
}
