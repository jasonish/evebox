// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::sqlite::has_table;
use crate::sqlite::info::Info;
use crate::sqlite::util::fts_create;
use deadpool_sqlite::CreatePoolError;
use rusqlite::{Connection, OpenFlags};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::{SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode};
use sqlx::sqlite::{SqliteConnection, SqliteSynchronous};
use sqlx::ConnectOptions;
use sqlx::Connection as _;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

pub(crate) struct ConnectionBuilder {
    pub filename: Option<PathBuf>,
}

impl ConnectionBuilder {
    pub fn filename<T: Into<PathBuf>>(filename: Option<T>) -> ConnectionBuilder {
        ConnectionBuilder {
            filename: filename.map(|filename| filename.into()),
        }
    }

    pub fn open(&self, create: bool) -> Result<Connection, rusqlite::Error> {
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        if let Some(filename) = &self.filename {
            debug!("Opening database {}", filename.display());
            rusqlite::Connection::open_with_flags(filename, flags)
        } else {
            rusqlite::Connection::open("file::memory:?cache=shared")
        }
    }

    pub async fn _open_sqlx_pool(&self, create: bool) -> Result<sqlx::SqlitePool, sqlx::Error> {
        open_sqlx_pool(self.filename.clone(), create).await
    }

    pub async fn open_sqlx_connection(
        &self,
        create: bool,
    ) -> Result<SqliteConnection, sqlx::Error> {
        open_sqlx_connection(self.filename.clone(), create).await
    }
}

fn sqlite_options() -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .journal_mode(SqliteJournalMode::Wal)
        .auto_vacuum(SqliteAutoVacuum::Full)
        .synchronous(SqliteSynchronous::Normal)
        .disable_statement_logging()
}

pub(crate) async fn open_sqlx_connection(
    path: Option<impl Into<PathBuf>>,
    create: bool,
) -> Result<SqliteConnection, sqlx::Error> {
    let path = path
        .map(|p| p.into())
        .unwrap_or_else(|| "file::memory:?cache=shared".into());

    let options = sqlite_options().filename(path).create_if_missing(create);

    SqliteConnection::connect_with(&options).await
}

pub(crate) async fn open_sqlx_pool(
    path: Option<impl Into<PathBuf>>,
    create: bool,
) -> Result<sqlx::Pool<sqlx::Sqlite>, sqlx::Error> {
    let path = path
        .map(|p| p.into())
        .unwrap_or_else(|| "file::memory:?cache=shared".into());

    let pool = SqlitePoolOptions::new()
        .min_connections(4)
        .max_connections(12);

    let options = sqlite_options().filename(path).create_if_missing(create);
    pool.connect_with(options).await
}

/// Open an SQLite connection pool with deadpool.
///
/// Fortunately SQLites default connection options are good enough as
/// deadpool does not provide a way to customize them. See
/// https://github.com/bikeshedder/deadpool/issues/214.
pub(crate) fn open_deadpool<P: Into<PathBuf>>(
    path: Option<P>,
) -> Result<deadpool_sqlite::Pool, CreatePoolError> {
    let path = path
        .map(|p| p.into())
        .unwrap_or_else(|| "file::memory:?cache=shared".into());
    let config = deadpool_sqlite::Config::new(path);
    let pool = config.create_pool(deadpool_sqlite::Runtime::Tokio1)?;
    Ok(pool)
}

pub(crate) async fn init_event_db2(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    let fresh_install = !has_table(conn, "events").await?;

    // Work-around as SQLx does not set the auto_vacuum pragma's in the correct order.
    if fresh_install {
        enable_auto_vacuum(conn).await?;
    }

    let mut tx = conn.begin().await?;
    let mut info = Info::new(&mut tx);

    let auto_vacuum = info.get_auto_vacuum().await?;
    debug!("Auto-vacuum: {auto_vacuum}");
    if auto_vacuum != 1 {
        warn!("Auto-vacuum is set to {}, expected 1", auto_vacuum);
    }

    let journal_mode = info.get_journal_mode().await?;
    debug!("Journal mode: {journal_mode}");
    if journal_mode != "wal" {
        warn!("Journal mode is set to {}, expected wal", journal_mode);
    }

    let synchronous = info.get_synchronous().await?;
    debug!("Synchronous: {synchronous}");
    if synchronous != 1 {
        warn!("Synchronous is set to {}, expected 1", synchronous);
    }

    async fn get_legacy_schema_version(conn: &mut sqlx::SqliteConnection) -> Option<i64> {
        if let Ok(true) = has_table(&mut *conn, "_sqlx_migrations").await {
            // Already migrated, return None.
            return None;
        }

        // Check for a version from refinery.
        let version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM refinery_schema_history")
                .fetch_optional(&mut *conn)
                .await
                .unwrap_or(None);
        if version.is_some() {
            return version;
        }

        // Check for a pre-refinery version.
        sqlx::query_scalar("SELECT MAX(version) FROM schema")
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None)
    }

    if let Some(version) = get_legacy_schema_version(&mut tx).await {
        info!("Found legacy schema version, migrating to SQLx");
        sqlx::query(
            r#"
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            );"#,
        )
        .execute(&mut *tx)
        .await?;

        if version >= 1 {
            sqlx::query("INSERT INTO _sqlx_migrations VALUES(1,'Initial','2024-05-13 21:03:48',1,X'7989260019cd6c1e3dbca57459291c454cf44316d68295616adb6189a11c8c6622a755b826c749c9a36b329cc98b401a',5781999)").execute(&mut *tx).await?;
        }

        if version >= 2 {
            sqlx::query("INSERT INTO _sqlx_migrations VALUES(2,'Indices','2024-05-13 21:03:48',1,X'6221123362b2c80db449fd4df044f4e2026841a793be55deceeb9ff5f62c8681b47af587523a35379fd9636c641eada8',3657464);
").execute(&mut *tx).await?;
        }

        if version >= 3 {
            sqlx::query("INSERT INTO _sqlx_migrations VALUES(3,'RemoveFTS','2024-05-13 21:03:48',1,X'8c6dfe03f4086331b68d1138e62ba4794d9d76493ee66cde36dd8b25b541991820e9e00751de0779d639d35dda732e3b',3716569);
").execute(&mut *tx).await?;
        }

        if version >= 4 {
            sqlx::query("INSERT INTO _sqlx_migrations VALUES(4,'EventsSourceValues','2024-05-13 21:03:48',1,X'cea6316146daaa99181ded5e6ebc1cdb3e8358ef88a58e0bd7e772ebd2c0ca0ca2bc9b405951f06a36f9235307cba1fd',3515654);
").execute(&mut *tx).await?;
        }

        // Mayber later...
        //
        // sqlx::query("DROP TABLE refinery_schema_history")
        //     .execute(&mut *tx)
        //     .await?;
    }

    sqlx::migrate!("resources/sqlite/migrations")
        .run(&mut *tx)
        .await
        .unwrap();

    if let Some(indexes) = crate::resource::get_string("sqlite/Indexes.sql") {
        info!("Updating SQLite indexes");

        if let Err(err) = sqlx::query(&indexes).execute(&mut *tx).await {
            error!("Failed to update SQLite indexes: {err}");
        }
    } else {
        error!("Failed to find sqlite/Indexes.sql");
    }

    if fresh_install {
        info!("Enabling FTS");
        fts_create(&mut tx).await?;
    } else if !has_table(&mut tx, "fts").await? {
        info!("FTS not enabled, consider enabling for query performance improvements");
    }

    let _ = tx.commit().await;

    Ok(())
}

async fn enable_auto_vacuum(conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA auto_vacuum = 1; VACUUM")
        .execute(conn)
        .await?;
    Ok(())
}
