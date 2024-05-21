// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub mod builder;
pub mod configrepo;
pub mod connection;
pub mod eventrepo;
pub mod importer;
pub(crate) mod info;
pub mod retention;
pub mod util;

pub(crate) use connection::ConnectionBuilder;
use sqlx::{sqlite::SqliteArguments, SqliteConnection, SqlitePool};
use tracing::error;

pub(crate) async fn has_table(
    conn: &mut SqliteConnection,
    name: &str,
) -> Result<bool, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(name)
            .fetch_one(&mut *conn)
            .await?;
    Ok(count > 0)
}

async fn log_query_plan<'a>(pool: &SqlitePool, tag: &str, sql: &str, args: SqliteArguments<'a>) {
    let rows: Result<Vec<(i64, i64, i64, String)>, sqlx::Error> =
        sqlx::query_as_with(&format!("explain query plan {}", &sql), args.clone())
            .fetch_all(pool)
            .await;
    match rows {
        Err(err) => {
            error!(
                "query-plan:{tag}: Failed to explain query plan: {}: sql={}",
                err, sql
            );
        }
        Ok(rows) => {
            tracing::info!("query-plan:{tag} for sql={sql}");
            for row in rows {
                tracing::info!("query-plan:{tag}: {}", row.3);
            }
        }
    }
}
