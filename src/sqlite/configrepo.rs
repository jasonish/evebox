// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use rusqlite::params;

use crate::prelude::*;
use crate::sqlite::ConnectionBuilder;

#[derive(thiserror::Error, Debug)]
pub enum ConfigRepoError {
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
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct User {
    pub uuid: String,
    pub username: String,
}

pub struct ConfigRepo {
    pub db: Arc<Mutex<rusqlite::Connection>>,
}

impl ConfigRepo {
    pub fn new(filename: Option<&PathBuf>) -> Result<Self, ConfigRepoError> {
        let mut conn = ConnectionBuilder::filename(filename).open()?;
        init_db(&mut conn)?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn get_user_by_username_password(
        &self,
        username: &str,
        password_in: &str,
    ) -> Result<User, ConfigRepoError> {
        let username = username.to_string();
        let password_in = password_in.to_string();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT uuid, username, password FROM users WHERE username = ?1")?;
            let mut rows = stmt.query(params![username])?;
            if let Some(row) = rows.next()? {
                let uuid: String = row.get(0)?;
                let username: String = row.get(1)?;
                let password_hash: String = row.get(2)?;
                if bcrypt::verify(password_in, &password_hash)? {
                    Ok(User {
                        uuid: uuid,
                        username: username,
                    })
                } else {
                    Err(ConfigRepoError::BadPassword(username))
                }
            } else {
                Err(ConfigRepoError::UsernameNotFound(username))
            }
        })
        .await?
    }

    pub fn get_user_by_name(&self, username: &str) -> Result<User, ConfigRepoError> {
        let conn = self.db.lock().unwrap();
        let user = conn
            .query_row(
                "SELECT uuid, username FROM users WHERE username = ?",
                params![username],
                |row| {
                    Ok(User {
                        uuid: row.get(0)?,
                        username: row.get(1)?,
                    })
                },
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    ConfigRepoError::NoUser(username.to_string())
                }
                _ => err.into(),
            })?;
        Ok(user)
    }

    pub fn get_users(&self) -> Result<Vec<User>, ConfigRepoError> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT uuid, username FROM users")?;
        let rows = stmt.query_map(params![], |row| {
            Ok(User {
                uuid: row.get(0)?,
                username: row.get(1)?,
            })
        })?;
        let mut users = Vec::new();
        for row in rows {
            users.push(row?);
        }
        Ok(users)
    }

    pub fn add_user(&self, username: &str, password: &str) -> Result<String, ConfigRepoError> {
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        let user_id = uuid::Uuid::new_v4().to_string();
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO users (uuid, username, password) VALUES (?, ?, ?)",
            params![user_id, username, password_hash],
        )?;
        tx.commit()?;
        Ok(user_id)
    }

    pub fn remove_user(&self, username: &str) -> Result<usize, ConfigRepoError> {
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction()?;
        let n = tx.execute("DELETE FROM users WHERE username = ?", params![username])?;
        tx.commit()?;
        Ok(n)
    }

    pub fn update_password_by_id(&self, id: &str, password: &str) -> Result<bool, ConfigRepoError> {
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction()?;
        let n = tx.execute(
            "UPDATE users SET password = ? where uuid = ?",
            params![password_hash, id],
        )?;
        tx.commit()?;
        Ok(n > 0)
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
            let now = chrono::Local::now();
            println!("{}", now);
            if let Err(err) = db.execute(
                "INSERT INTO refinery_schema_history VALUES (?, ?, ?, ?)",
                params![1, "Initial", now.to_rfc3339(), "866978575299187291"],
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
