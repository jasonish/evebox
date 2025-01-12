// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use crate::sqlite::prelude::*;

use std::path::Path;

use crate::datetime::DateTime;
use crate::sqlite::has_table;

#[derive(thiserror::Error, Debug)]
pub(crate) enum ConfigDbError {
    #[error("username not found: {0}")]
    UsernameNotFound(String),
    #[error("bad password for user: {0}")]
    BadPassword(String),
    #[error("bcrypt error: {0}")]
    BcryptError(#[from] bcrypt::BcryptError),
    #[error("join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("user does not exist: {0}")]
    NoUser(String),
    #[error("sql error: {0}")]
    SqlxError(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, FromRow)]
pub(crate) struct FilterRow {
    pub id: i64,
    pub filter: sqlx::types::Json<FilterEntry>,
    pub user_id: i64,
    pub enabled: bool,
    pub created_at: crate::datetime::ChronoDateTime,
    pub updated_at: crate::datetime::ChronoDateTime,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub(crate) struct FilterEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_ip: Option<String>,
    pub signature_id: i64,

    // Only here for ease of the API, should be removed as it has its
    // own field in the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct User {
    pub uuid: String,
    pub username: String,
}

#[derive(Clone)]
pub(crate) struct ConfigDb {
    pub(crate) pool: SqlitePool,
}

impl ConfigDb {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_user_by_username_password(
        &self,
        username: &str,
        password_in: &str,
    ) -> Result<User, ConfigDbError> {
        let query = sqlx::query::<sqlx::Sqlite>(
            "SELECT uuid, username, password FROM users WHERE username = ?",
        )
        .bind(username);
        if let Some(row) = query.fetch_optional(&self.pool).await? {
            let uuid: String = row.try_get(0)?;
            let username: String = row.try_get(1)?;
            let password_hash: String = row.try_get(2)?;
            if bcrypt::verify(password_in, &password_hash)? {
                return Ok(User { uuid, username });
            } else {
                return Err(ConfigDbError::BadPassword(username));
            }
        }

        Err(ConfigDbError::UsernameNotFound(username.to_string()))
    }

    pub async fn get_user_by_name(&self, username: &str) -> Result<User, ConfigDbError> {
        let row = sqlx::query("SELECT uuid, username FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row {
            Ok(User {
                uuid: row.try_get("uuid")?,
                username: row.try_get("username")?,
            })
        } else {
            Err(ConfigDbError::NoUser(username.to_string()))
        }
    }

    pub async fn has_users(&self) -> Result<bool, ConfigDbError> {
        let (count,): (u64,) = sqlx::query_as("SELECT count(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    pub async fn get_users(&self) -> Result<Vec<User>, ConfigDbError> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT uuid, username FROM users")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| User {
                uuid: row.0,
                username: row.1,
            })
            .collect())
    }

    pub async fn add_user(&self, username: &str, password: &str) -> Result<String, ConfigDbError> {
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        let user_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO users (uuid, username, password) VALUES (?, ?, ?)")
            .bind(&user_id)
            .bind(username)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;
        Ok(user_id)
    }

    pub async fn remove_user(&self, username: &str) -> Result<u64, ConfigDbError> {
        Ok(sqlx::query("DELETE FROM users WHERE username = ?")
            .bind(username)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    pub async fn update_password_by_id(
        &self,
        id: &str,
        password: &str,
    ) -> Result<bool, ConfigDbError> {
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        let result = sqlx::query("UPDATE users SET password = ? WHERE uuid = ?")
            .bind(&password_hash)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn save_session(
        &self,
        token: &str,
        uuid: &str,
        expires: i64,
    ) -> Result<(), ConfigDbError> {
        let sql = "INSERT INTO sessions (token, uuid, expires_at) VALUES (?, ?, ?)";
        sqlx::query(sql)
            .bind(token)
            .bind(uuid)
            .bind(expires)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_session(&self, token: &str) -> Result<(), ConfigDbError> {
        let sql = "DELETE FROM sessions WHERE token = ?";
        sqlx::query(sql).bind(token).execute(&self.pool).await?;
        Ok(())
    }

    async fn expire_sessions(&self) -> Result<u64, ConfigDbError> {
        let now = DateTime::now().to_seconds();
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn get_user_by_session(&self, token: &str) -> Result<Option<User>, ConfigDbError> {
        let sql = r#"
            SELECT users.uuid, users.username, sessions.expires_at
            FROM users 
            JOIN sessions ON 
            users.uuid = sessions.uuid
            WHERE sessions.token = ?"#;

        // TODO: Remove transaction, not needed here but used as an example.
        let mut tx = self.pool.begin().await?;
        if let Some(row) = sqlx::query(sql)
            .bind(token)
            .fetch_optional(&mut *tx)
            .await?
        {
            let uuid: String = row.try_get("uuid")?;
            let username: String = row.try_get("username")?;
            let expires_at: i64 = row.try_get("expires_at")?;

            let now = DateTime::now().to_seconds();
            if now > expires_at {
                match self.expire_sessions().await {
                    Ok(n) => {
                        if n > 0 {
                            info!("Expired {} sessions", n);
                        }
                    }
                    Err(err) => {
                        error!("Failed to expire sessions: {:?}", err);
                    }
                }
                return Ok(None);
            }
            tx.commit().await?;
            return Ok(Some(User { uuid, username }));
        }
        Ok(None)
    }

    pub(crate) async fn get_filters(&self) -> Result<Vec<FilterRow>> {
        let sql = "SELECT * FROM filters";
        let rows: Vec<FilterRow> = sqlx::query_as(sql).fetch_all(&self.pool).await?;
        Ok(rows)
    }
}

async fn get_legacy_version(conn: &mut SqliteConnection) -> Option<u8> {
    let version = sqlx::query_scalar("SELECT MAX(version) FROM refinery_schema_history")
        .fetch_one(&mut *conn)
        .await
        .ok();
    if version.is_some() {
        return version;
    }

    sqlx::query_scalar("SELECT MAX(version) FROM schema")
        .fetch_one(&mut *conn)
        .await
        .ok()
}

/// Initialize the configuration/administrative database, apply
/// migrations as needed.
async fn init_db(db: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    let has_slqx_migrations = has_table(&mut *tx, "_sqlx_migrations").await?;
    let legacy_version = get_legacy_version(&mut tx).await;

    if !has_slqx_migrations {
        if let Some(version) = legacy_version {
            let sql = r#"
                        CREATE TABLE _sqlx_migrations (
                            version BIGINT PRIMARY KEY,
                            description TEXT NOT NULL,
                            installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                            success BOOLEAN NOT NULL,
                            checksum BLOB NOT NULL,
                            execution_time BIGINT NOT NULL
                    );"#;
            sqlx::query(sql).execute(&mut *tx).await?;

            let rows = &[
                "INSERT INTO _sqlx_migrations VALUES(1,'Initial','2024-05-14 22:07:37',1,X'3178dae65749760972807044cd00fc973daf8e325e0bbdd2491faad1f0357ba1e7943db275d7283fc791e9f9495e769c',2046643)",
                "INSERT INTO _sqlx_migrations VALUES(2,'Sessions','2024-05-14 22:07:37',1,X'83b06692857c8f61cfc2d26ab18ed1207ca699213b06126b0aeb404219ff8c73b32439f15aebf44d30a5d972eca8546d',1180689)",
            ];

            if version >= 1 {
                debug!("Inserting fake configuration database migration for version 1");
                sqlx::query(rows[0]).execute(&mut *tx).await?;
            }
            if version >= 2 {
                debug!("Inserting fake configuration database migration for version 2");
                sqlx::query(rows[1]).execute(&mut *tx).await?;
            }
        }
    }

    debug!("Applying configuration/admin database migrations");
    sqlx::migrate!("resources/configdb/migrations")
        .run(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

/// Open and initialize the configuration database, returning a
/// ConfigRepo.
pub(crate) async fn open(filename: Option<&Path>) -> Result<ConfigDb, sqlx::Error> {
    info!(
        "Opening configuration database {}",
        filename
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ":memory:".to_string())
    );
    let pool = crate::sqlite::connection::open_pool(filename, true).await?;
    let mut conn = pool.acquire().await?;
    init_db(&mut conn).await?;
    Ok(ConfigDb::new(pool))
}

pub(crate) async fn open_connection_in_directory(
    directory: &Path,
) -> Result<sqlx::SqliteConnection, sqlx::Error> {
    let filename = directory.join("config.sqlite");
    info!("Opening configuration database file {}", filename.display());
    let mut conn = crate::sqlite::connection::open_connection(Some(&filename), true).await?;
    init_db(&mut conn).await?;
    Ok(conn)
}
