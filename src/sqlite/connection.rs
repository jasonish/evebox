// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::sqlite::has_table;
use crate::sqlite::info::Info;
use crate::sqlite::util::fts_create;
use regex::Regex;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::{SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode};
use sqlx::sqlite::{SqliteConnection, SqliteSynchronous};
use sqlx::Connection as _;
use sqlx::SqlitePool;
use std::collections::HashSet;
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

    pub fn open_with_rusqlite(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = if let Some(filename) = &self.filename {
            rusqlite::Connection::open_with_flags(filename, flags)?
        } else {
            //rusqlite::Connection::open("file::memory:?cache=shared")?
            unreachable!()
        };

        conn.pragma_query(None, "journal_mode", |row| {
            let mode: String = row.get(0)?;
            info!("Rusqlite connection: journal_mode={}", mode);
            Ok(())
        })?;

        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_query(None, "synchronous", |row| {
            let synchronous: i64 = row.get(0)?;
            info!("Rusqlite connection: synchronous={}", synchronous);
            Ok(())
        })?;

        conn.pragma_query(None, "auto_vacuum", |row| {
            let mode: i64 = row.get(0)?;
            info!("Rusqlite connection: auto_vacuum={}", mode);
            Ok(())
        })?;

        Ok(conn)
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
    let options = sqlite_options()
        .filename(path)
        .create_if_missing(create)
        .optimize_on_close(true, 300);
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
        .min_connections(12)
        .max_connections(30)
        .max_lifetime(Some(std::time::Duration::from_secs(600)))
        .after_release(|conn, _| Box::pin(async move { after_release(conn).await }))
        .after_connect(|conn, _| Box::pin(async move { after_connect(conn).await }));

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

async fn after_release(conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
    // Remove any progress handlers.
    conn.lock_handle().await?.remove_progress_handler();
    Ok(true)
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

    if fresh_install {
        update_indexes(&mut tx).await?;
    }

    if fresh_install {
        info!("Enabling FTS");
        fts_create(&mut tx).await?;
    } else if !has_table(&mut *tx, "fts").await? {
        info!("FTS not enabled, consider enabling for query performance improvements");
    }

    match check_indexes(&mut tx).await {
        Ok(false) => {
            debug!("Event table indexes OK");
        }
        Ok(true) => {
            warn!("Event table indexes out of table, please consider updating the indexes");
        }
        Err(err) => {
            warn!("Failed to validate event table indexes: {:?}", err);
        }
    }

    tx.commit().await?;

    Ok(())
}

pub(crate) async fn update_indexes(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    if let Some(indexes) = crate::resource::get_string("sqlite/Indexes.sql") {
        // The indexes that exist in the database.
        let current_indexes = get_current_indexes(conn).await?;

        // The known indexes from Indexes.sql.
        let known_indexes = parse_index_names(&indexes);

        for index in &current_indexes {
            if !known_indexes.contains(index) {
                info!("Removing obsolete index {index}");
                if let Err(err) = drop_index(conn, index).await {
                    error!("Failed to drop index {}: {:?}", index, err);
                }
            }
        }

        info!("Updating SQLite indexes");
        if let Err(err) = sqlx::query(&indexes).execute(&mut *conn).await {
            error!("Failed to update SQLite indexes: {err}");
        }
    } else {
        error!("Failed to find sqlite/Indexes.sql");
    }

    Ok(())
}

async fn drop_index(conn: &mut SqliteConnection, index: &str) -> anyhow::Result<()> {
    sqlx::query(&format!("DROP INDEX {index}"))
        .execute(conn)
        .await?;
    Ok(())
}

fn parse_index_names(sql: &str) -> HashSet<String> {
    let mut indexes = HashSet::new();

    let re = Regex::new(r"CREATE INDEX IF NOT EXISTS (\w+)").unwrap();
    for line in sql.lines() {
        if let Some(caps) = re.captures(line) {
            if let Some(cap) = caps.get(1) {
                indexes.insert(cap.as_str().to_string());
            }
        }
    }

    indexes
}

async fn get_current_indexes(conn: &mut SqliteConnection) -> anyhow::Result<Vec<String>> {
    let mut indexes = vec![];

    let rows: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'index'")
            .fetch_all(conn)
            .await?;

    for index in &rows {
        if index.starts_with("sqlite") {
            continue;
        }
        indexes.push(index.to_string());
    }

    Ok(indexes)
}

async fn check_indexes(conn: &mut SqliteConnection) -> anyhow::Result<bool> {
    let current_indexes = get_current_indexes(conn)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to get current indexes: {:?}", err))?;
    let current_indexes: HashSet<String> = HashSet::from_iter(current_indexes.iter().cloned());
    let indexes_sql = crate::resource::get_string("sqlite/Indexes.sql")
        .ok_or_else(|| anyhow::anyhow!("Failed to find sqlite/Indexes.sql"))?;
    let known_indexes = parse_index_names(&indexes_sql);

    let mut dirty = false;

    for index in &current_indexes {
        if !known_indexes.contains(index) {
            warn!("Events table contains obsolete or unknown index {index}");
            dirty = true;
        }
    }

    for index in &known_indexes {
        if !current_indexes.contains(index) {
            warn!("Events table is missing index {index}");
            dirty = true;
        }
    }

    Ok(dirty)
}
