// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::sqlite::has_table;
use crate::sqlite::info::Info;
use crate::sqlite::util::fts_create;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::{SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode};
use sqlx::sqlite::{SqliteConnection, SqliteSynchronous};
use sqlx::Connection as _;
use sqlx::SqlitePool;
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

    pub async fn open_connection(&self, create: bool) -> Result<SqliteConnection, sqlx::Error> {
        open_connection(self.filename.clone(), create).await
    }

    pub async fn open_pool(&self, create: bool) -> Result<SqlitePool, sqlx::Error> {
        open_pool(self.filename.clone(), create).await
    }
}

fn sqlite_options() -> SqliteConnectOptions {
    use sqlx::ConnectOptions;

    let mut options = SqliteConnectOptions::new()
        .journal_mode(SqliteJournalMode::Wal)
        .auto_vacuum(SqliteAutoVacuum::Full)
        .synchronous(SqliteSynchronous::Normal);

    if std::env::var("EVEBOX_SQLX_STATEMENT_LOGGING").is_ok() {
        options = options.log_statements(log::LevelFilter::Debug);
    } else {
        options = options.disable_statement_logging();
    }

    // 5 seconds just isn't long enough when we expect possibly long
    // lockout times.
    options.busy_timeout(std::time::Duration::from_secs(86400))
}

pub(crate) async fn open_connection(
    path: Option<impl Into<PathBuf>>,
    create: bool,
) -> Result<SqliteConnection, sqlx::Error> {
    let path = path
        .map(|p| p.into())
        .unwrap_or_else(|| "file::memory:?cache=shared".into());

    let options = sqlite_options().filename(path).create_if_missing(create);

    let mut conn = SqliteConnection::connect_with(&options).await?;

    after_connect(&mut conn).await?;

    Ok(conn)
}

pub(crate) async fn open_pool(
    path: Option<impl Into<PathBuf>>,
    create: bool,
) -> Result<SqlitePool, sqlx::Error> {
    let path = path
        .map(|p| p.into())
        .unwrap_or_else(|| "file::memory:?cache=shared".into());

    let pool = SqlitePoolOptions::new()
        .min_connections(4)
        .max_connections(12)
        .after_connect(|conn, _meta| Box::pin(async move { after_connect(conn).await }));

    let options = sqlite_options()
        .filename(path)
        .create_if_missing(create)
        .optimize_on_close(true, Some(300));

    pool.connect_with(options).await
}

async fn after_connect(conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    let n: u64 = sqlx::query_scalar("PRAGMA journal_size_limit = 32000000")
        .fetch_one(&mut *conn)
        .await?;
    debug!("PRAGMA journal_size_limit returned {}", n);

    // Disable as it can be very slow.
    sqlx::query("PRAGMA secure_delete = off")
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub(crate) async fn init_event_db(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    let fresh_install = !has_table(&mut *conn, "events").await?;

    // Work-around as SQLx does not set the auto_vacuum pragma's in the correct order.
    if fresh_install {
        crate::sqlite::util::enable_auto_vacuum(&mut *conn).await?;
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

    // Skip if EVEBOX_SKIP_INBOX_UPDATE=yes.
    if std::env::var("EVEBOX_SKIP_INDEX_UPDATE").is_err() {
        if let Some(indexes) = crate::resource::get_string("sqlite/Indexes.sql") {
            info!("Updating SQLite indexes");

            if let Err(err) = sqlx::query(&indexes).execute(&mut *tx).await {
                error!("Failed to update SQLite indexes: {err}");
            }
        } else {
            error!("Failed to find sqlite/Indexes.sql");
        }
    }

    if fresh_install {
        info!("Enabling FTS");
        fts_create(&mut tx).await?;
    } else if !has_table(&mut *tx, "fts").await? {
        info!("FTS not enabled, consider enabling for query performance improvements");
    }

    let _ = tx.commit().await;

    Ok(())
}
