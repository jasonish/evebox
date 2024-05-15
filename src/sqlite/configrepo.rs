// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::path::Path;

use serde::Serialize;
use sqlx::Row;
use sqlx::SqlitePool;
use tracing::debug;
use tracing::error;
use tracing::info;

use super::has_table;

#[derive(thiserror::Error, Debug)]
pub(crate) enum ConfigRepoError {
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

#[derive(Debug, Clone, Serialize)]
pub(crate) struct User {
    pub uuid: String,
    pub username: String,
}

#[derive(Clone)]
pub(crate) struct ConfigRepo {
    pool: SqlitePool,
}

impl ConfigRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_user_by_username_password(
        &self,
        username: &str,
        password_in: &str,
    ) -> Result<User, ConfigRepoError> {
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
                return Err(ConfigRepoError::BadPassword(username));
            }
        }

        Err(ConfigRepoError::UsernameNotFound(username.to_string()))
    }

    pub async fn get_user_by_name(&self, username: &str) -> Result<User, ConfigRepoError> {
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
            Err(ConfigRepoError::NoUser(username.to_string()))
        }
    }

    pub async fn has_users(&self) -> Result<bool, ConfigRepoError> {
        let (count,): (u64,) = sqlx::query_as("SELECT count(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    pub async fn get_users(&self) -> Result<Vec<User>, ConfigRepoError> {
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

    pub async fn add_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, ConfigRepoError> {
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

    pub async fn remove_user(&self, username: &str) -> Result<u64, ConfigRepoError> {
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
    ) -> Result<bool, ConfigRepoError> {
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
    ) -> Result<(), ConfigRepoError> {
        let sql = "INSERT INTO sessions (token, uuid, expires_at) VALUES (?, ?, ?)";
        sqlx::query(sql)
            .bind(token)
            .bind(uuid)
            .bind(expires)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn expire_sessions(&self) -> Result<u64, ConfigRepoError> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let result = sqlx::query("DELETE FROM sessions WHERE expires AT < ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn get_user_by_session(&self, token: &str) -> Result<Option<User>, ConfigRepoError> {
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

            let now = time::OffsetDateTime::now_utc().unix_timestamp();
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
}

async fn get_legacy_version(db: SqlitePool) -> Option<u8> {
    let version = sqlx::query_scalar("SELECT MAX(version) FROM refinery_schema_history")
        .fetch_one(&db)
        .await
        .ok();
    if version.is_some() {
        return version;
    }

    sqlx::query_scalar("SELECT MAX(version) FROM schema")
        .fetch_one(&db)
        .await
        .ok()
}

/// Initialize the configuration/administrative database, apply
/// migrations as needed.
async fn init_db(db: SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    if has_table(&mut tx, "_sqlx_migrations").await? {
        // Nothing to do.
        return Ok(());
    }
    let legacy_version = get_legacy_version(db.clone()).await;

    if let Some(version) = legacy_version {
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

    debug!("Applying configuration/admin database migrations");
    sqlx::migrate!("resources/configdb/migrations")
        .run(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

/// Open and initialize the configuration database, returning a
/// ConfigRepo.
pub(crate) async fn open(filename: Option<&Path>) -> Result<ConfigRepo, sqlx::Error> {
    info!(
        "Opening configuration database {}",
        filename
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ":memory:".to_string())
    );
    let pool = crate::sqlite::connection::open_pool(filename, true).await?;
    init_db(pool.clone()).await?;
    Ok(ConfigRepo::new(pool))
}
