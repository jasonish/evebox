// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use rusqlite::params;
use serde::Serialize;
use sqlx::Row;
use time::format_description::well_known::Rfc3339;
use tracing::debug;
use tracing::error;
use tracing::info;

use crate::sqlite::ConnectionBuilder;

#[derive(thiserror::Error, Debug)]
pub(crate) enum ConfigRepoError {
    #[error("username not found: {0}")]
    UsernameNotFound(String),
    #[error("bad password for user: {0}")]
    BadPassword(String),
    #[error("sqlite error: {0}")]
    SqliteError(#[from] rusqlite::Error),
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

pub(crate) struct ConfigRepo {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl ConfigRepo {
    pub async fn new(filename: Option<&PathBuf>) -> Result<Self, ConfigRepoError> {
        let mut conn = ConnectionBuilder::filename(filename).open(true)?;
        init_db(&mut conn)?;
        Ok(Self {
            pool: crate::sqlite::connection::open_sqlx_pool(filename, false).await?,
        })
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

pub fn init_db(db: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let version = db
        .query_row("select max(version) from schema", params![], |row| {
            let version: i64 = row.get(0).unwrap();
            Ok(version)
        })
        .unwrap_or(-1);
    if version == 1 {
        // We may have to provide the refinery table, unless it was already created.
        debug!("SQLite configuration DB at v1, checking if setup required for Refinery migrations");
        let fake_refinery_setup = "CREATE TABLE refinery_schema_history(
            version INT4 PRIMARY KEY,
            name VARCHAR(255),
            applied_on VARCHAR(255),
            checksum VARCHAR(255))";
        if db.execute(fake_refinery_setup, params![]).is_ok() {
            let now = time::OffsetDateTime::now_utc();
            let formatted_now = now.format(&Rfc3339).unwrap();
            if let Err(err) = db.execute(
                "INSERT INTO refinery_schema_history VALUES (?, ?, ?, ?)",
                params![1, "Initial", formatted_now, "866978575299187291"],
            ) {
                error!("Failed to initialize schema history table: {:?}", err);
            } else {
                debug!("SQLite configuration DB now setup for refinery migrations");
            }
        } else {
            debug!("Refinery migrations already exist for SQLite configuration DB");
        }
    }

    embedded::migrations::runner().run(db).unwrap();
    Ok(())
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./resources/configdb/migrations");
}
