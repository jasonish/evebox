// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use crate::sqlite::prelude::*;

use std::collections::HashMap;
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
    #[error("an agent key named {0:?} already exists, remove it first")]
    AgentKeyNameInUse(String),
    #[error("agent name {0:?} is reserved for the server-local PCAP spool")]
    AgentNameReserved(String),
    #[error("agent name must not be empty")]
    EmptyAgentName,
    #[error("agent name is limited to {0} bytes")]
    AgentNameTooLong(usize),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, FromRow)]
pub(crate) struct FilterRow {
    pub id: i64,
    pub filter: sqlx::types::Json<EventFilter>,
    pub user_id: i64,
    pub enabled: bool,
    pub created_at: crate::datetime::ChronoDateTime,
    pub updated_at: crate::datetime::ChronoDateTime,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FilterAction {
    Archive,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FilterOperator {
    Eq,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub(crate) struct FilterCondition {
    pub field: String,
    pub op: FilterOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub(crate) struct EventFilter {
    pub action: FilterAction,
    pub conditions: Vec<FilterCondition>,
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

impl From<&FilterEntry> for EventFilter {
    fn from(entry: &FilterEntry) -> Self {
        let mut conditions = Vec::new();
        let mut push_string = |field: &str, value: &Option<String>| {
            if let Some(value) = value {
                conditions.push(FilterCondition {
                    field: field.to_string(),
                    op: FilterOperator::Eq,
                    value: value.clone().into(),
                });
            }
        };
        push_string("host", &entry.sensor);
        push_string("src_ip", &entry.src_ip);
        push_string("dest_ip", &entry.dest_ip);
        conditions.push(FilterCondition {
            field: "alert.signature_id".to_string(),
            op: FilterOperator::Eq,
            value: entry.signature_id.into(),
        });
        Self {
            action: FilterAction::Archive,
            conditions,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct User {
    pub uuid: String,
    pub username: String,
}

/// Prefix on every agent key, making a leaked key greppable and
/// identifiable.
pub(crate) const AGENT_KEY_PREFIX: &str = "eba_";

/// A long-lived agent credential.
///
/// Keys are high-entropy random bearer tokens stored as-is, not hashed, so
/// an operator can re-show a key when re-provisioning a sensor. A leaked key
/// grants agent impersonation on the control channel, not access to stored
/// data or the browser UI. Protect the configuration database file like
/// agent.yaml.
#[derive(Debug, Clone, Serialize, FromRow)]
pub(crate) struct AgentKey {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub created_at: crate::datetime::ChronoDateTime,
    pub last_seen: Option<crate::datetime::ChronoDateTime>,
}

fn generate_agent_key() -> String {
    use base64::prelude::*;
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    format!("{AGENT_KEY_PREFIX}{}", BASE64_URL_SAFE_NO_PAD.encode(buf))
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnabledWithValue {
    pub enabled: bool,
    pub value: u64,
}

#[derive(Clone, Debug)]
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
        let (count,): (u64,) =
            sqlx::query_as("SELECT count(*) FROM users WHERE username != '__system__'")
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

    pub(crate) async fn kv_get_config(&self) -> Result<HashMap<String, serde_json::Value>> {
        let sql = "SELECT key, value FROM kv WHERE key LIKE 'config.%'";
        let rows: Vec<(String, serde_json::Value)> =
            sqlx::query_as(sql).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().collect())
    }

    pub(crate) async fn kv_get_config_as_json(
        &self,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let sql = "SELECT value FROM kv WHERE key = ?";
        let value: Option<serde_json::Value> = sqlx::query_scalar(sql)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value)
    }

    pub(crate) async fn kv_get_config_as_t<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let value = self.kv_get_config_as_json(key).await?;
        Ok(value.map(|v| serde_json::from_value(v)).transpose()?)
    }

    pub(crate) async fn kv_set_config(
        &self,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        let sql = "INSERT OR REPLACE INTO kv (key, value) VALUES (?, ?)";
        sqlx::query(sql)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Create an agent key. The generated key is returned in the row and
    /// remains re-showable through `list_agent_keys`. One key per name;
    /// replace a key by removing it and adding a new one.
    pub(crate) async fn add_agent_key(&self, name: &str) -> Result<AgentKey, ConfigDbError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ConfigDbError::EmptyAgentName);
        }
        if name.len() > crate::server::agents::MAX_AGENT_NAME_BYTES {
            return Err(ConfigDbError::AgentNameTooLong(
                crate::server::agents::MAX_AGENT_NAME_BYTES,
            ));
        }
        if name == crate::server::agents::LOCAL_PCAP_SOURCE_NAME {
            return Err(ConfigDbError::AgentNameReserved(name.to_string()));
        }
        let key = generate_agent_key();
        let result = sqlx::query("INSERT INTO agent_keys (name, key) VALUES (?, ?)")
            .bind(name)
            .bind(&key)
            .execute(&self.pool)
            .await;
        if let Err(sqlx::Error::Database(err)) = &result
            && err.is_unique_violation()
        {
            return Err(ConfigDbError::AgentKeyNameInUse(name.to_string()));
        }
        result?;
        let row = sqlx::query_as("SELECT * FROM agent_keys WHERE key = ?")
            .bind(&key)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    /// Look up an agent key by its presented value.
    pub(crate) async fn verify_agent_key(
        &self,
        key: &str,
    ) -> Result<Option<AgentKey>, ConfigDbError> {
        if !key.starts_with(AGENT_KEY_PREFIX) {
            return Ok(None);
        }
        let row = sqlx::query_as("SELECT * FROM agent_keys WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub(crate) async fn list_agent_keys(&self) -> Result<Vec<AgentKey>, ConfigDbError> {
        let rows = sqlx::query_as("SELECT * FROM agent_keys ORDER BY name, id")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Look up an agent key row by its row id.
    pub(crate) async fn get_agent_key_by_id(
        &self,
        id: i64,
    ) -> Result<Option<AgentKey>, ConfigDbError> {
        let row = sqlx::query_as("SELECT * FROM agent_keys WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Delete the key with this name, returning false when no key has the
    /// name.
    pub(crate) async fn remove_agent_key(&self, name: &str) -> Result<bool, ConfigDbError> {
        let result = sqlx::query("DELETE FROM agent_keys WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete the key with this row id, returning false when no key has
    /// the id.
    pub(crate) async fn remove_agent_key_by_id(&self, id: i64) -> Result<bool, ConfigDbError> {
        let result = sqlx::query("DELETE FROM agent_keys WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub(crate) async fn touch_agent_key(&self, id: i64) -> Result<(), ConfigDbError> {
        sqlx::query("UPDATE agent_keys SET last_seen = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
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

    if !has_slqx_migrations && let Some(version) = legacy_version {
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

#[cfg(test)]
mod tests {
    use super::*;

    // In-memory SQLite connections share a process-global cache, so every
    // test gets its own database file.
    async fn test_db() -> (tempfile::TempDir, ConfigDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = open(Some(&dir.path().join("config.sqlite"))).await.unwrap();
        (dir, db)
    }

    #[test]
    fn condition_format_represents_future_filter_shapes() {
        let src_ip: EventFilter = serde_json::from_value(json!({
            "action": "archive",
            "conditions": [
                {"field": "src_ip", "op": "eq", "value": "192.0.2.10"},
            ],
        }))
        .unwrap();
        assert_eq!(src_ip.conditions.len(), 1);

        let dns: EventFilter = serde_json::from_value(json!({
            "action": "archive",
            "conditions": [
                {"field": "alert.signature_id", "op": "eq", "value": 3301003},
                {"field": "dns.queries.rrname", "op": "eq", "value": "example.com"},
            ],
        }))
        .unwrap();
        assert_eq!(dns.conditions.len(), 2);
    }

    #[tokio::test]
    async fn legacy_filters_are_migrated() {
        let (_dir, db) = test_db().await;
        sqlx::query("INSERT INTO filters (user_id, filter) VALUES (0, ?)")
            .bind(
                r#"{"sensor":"fw-east","src_ip":"10.1.1.1","signature_id":3301003,"comment":"legacy"}"#,
            )
            .execute(&db.pool)
            .await
            .unwrap();

        sqlx::raw_sql(include_str!(
            "../../resources/configdb/migrations/0008_flexible_filters.sql"
        ))
        .execute(&db.pool)
        .await
        .unwrap();

        let rows = db.get_filters().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].comment.as_deref(), Some("legacy"));
        assert_eq!(
            rows[0].filter.conditions,
            vec![
                FilterCondition {
                    field: "host".to_string(),
                    op: FilterOperator::Eq,
                    value: "fw-east".into(),
                },
                FilterCondition {
                    field: "src_ip".to_string(),
                    op: FilterOperator::Eq,
                    value: "10.1.1.1".into(),
                },
                FilterCondition {
                    field: "alert.signature_id".to_string(),
                    op: FilterOperator::Eq,
                    value: 3301003.into(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn agent_keys_verify_and_touch() {
        let (_dir, db) = test_db().await;
        let added = db.add_agent_key("sensor-a").await.unwrap();
        assert!(added.key.starts_with(AGENT_KEY_PREFIX));
        assert!(added.last_seen.is_none());

        let found = db.verify_agent_key(&added.key).await.unwrap().unwrap();
        assert_eq!(found.id, added.id);
        assert_eq!(found.name, "sensor-a");

        db.touch_agent_key(added.id).await.unwrap();
        let touched = db.verify_agent_key(&added.key).await.unwrap().unwrap();
        assert!(touched.last_seen.is_some());

        // Unknown keys and non-agent-key bearer values are rejected.
        assert!(
            db.verify_agent_key("eba_underivable")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.verify_agent_key("xyz_wrongprefix")
                .await
                .unwrap()
                .is_none()
        );
        assert!(db.verify_agent_key("").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn removed_agent_keys_are_rejected() {
        let (_dir, db) = test_db().await;
        let added = db.add_agent_key("sensor-a").await.unwrap();
        assert!(db.remove_agent_key("sensor-a").await.unwrap());
        assert!(db.verify_agent_key(&added.key).await.unwrap().is_none());
        // Nothing is left to remove.
        assert!(!db.remove_agent_key("sensor-a").await.unwrap());
        assert!(!db.remove_agent_key("never-existed").await.unwrap());
    }

    #[tokio::test]
    async fn agent_key_names_are_unique() {
        let (_dir, db) = test_db().await;
        let first = db.add_agent_key("sensor-a").await.unwrap();
        assert!(matches!(
            db.add_agent_key("sensor-a").await,
            Err(ConfigDbError::AgentKeyNameInUse(_))
        ));

        // Removal frees the name for a replacement key.
        assert!(db.remove_agent_key("sensor-a").await.unwrap());
        let second = db.add_agent_key("sensor-a").await.unwrap();
        assert_ne!(first.key, second.key);

        let rows = db.list_agent_keys().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, second.key);
    }

    #[tokio::test]
    async fn agent_keys_by_id() {
        let (_dir, db) = test_db().await;
        let added = db.add_agent_key("sensor-a").await.unwrap();

        let found = db.get_agent_key_by_id(added.id).await.unwrap().unwrap();
        assert_eq!(found.name, "sensor-a");
        assert_eq!(found.key, added.key);
        assert!(
            db.get_agent_key_by_id(added.id + 1)
                .await
                .unwrap()
                .is_none()
        );

        assert!(db.remove_agent_key_by_id(added.id).await.unwrap());
        assert!(db.get_agent_key_by_id(added.id).await.unwrap().is_none());
        assert!(db.verify_agent_key(&added.key).await.unwrap().is_none());
        // Nothing is left to remove.
        assert!(!db.remove_agent_key_by_id(added.id).await.unwrap());
    }

    #[tokio::test]
    async fn agent_key_names_are_validated() {
        let (_dir, db) = test_db().await;
        assert!(matches!(
            db.add_agent_key("  ").await,
            Err(ConfigDbError::EmptyAgentName)
        ));
        assert!(matches!(
            db.add_agent_key(crate::server::agents::LOCAL_PCAP_SOURCE_NAME)
                .await,
            Err(ConfigDbError::AgentNameReserved(_))
        ));
        assert!(
            db.add_agent_key(&"a".repeat(crate::server::agents::MAX_AGENT_NAME_BYTES))
                .await
                .is_ok()
        );
        assert!(matches!(
            db.add_agent_key(&"b".repeat(crate::server::agents::MAX_AGENT_NAME_BYTES + 1))
                .await,
            Err(ConfigDbError::AgentNameTooLong(_))
        ));
    }
}
