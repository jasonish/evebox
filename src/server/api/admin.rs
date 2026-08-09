// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use axum::Form;
use axum::response::IntoResponse;
use axum::{Extension, Json, extract::Path};

use crate::server::autoarchive::AutoArchive;
use crate::server::{ServerContext, main::SessionExtractor};
use crate::sqlite::configdb::{AgentKey, EventFilter, FilterEntry, FilterRow};

pub(super) async fn update_ja4db(
    Extension(context): Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("API request to update JA4 database");
    match do_update(context).await {
        Ok(response) => {
            info!("JA4db updated");
            Ok(response)
        }
        Err(err) => {
            error!("Request to update JA4db failed: {err}");
            Err(err.into())
        }
    }
}

async fn do_update(context: Arc<ServerContext>) -> Result<Json<serde_json::Value>> {
    let mut conn = context.configdb.pool.begin().await?;
    let n = crate::commands::ja4db::updatedb(&mut conn).await?;
    conn.commit().await?;
    let response = json!({
        "entries": n,
    });
    Ok(Json(response))
}

/// Add auto-archive filters, but to be extended.
///
/// For now just use the FilterEntry from configdb as the form
/// type. But that may need to change as we extend this.
pub(super) async fn add_filter(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(mut entry): Form<FilterEntry>,
) -> Result<impl IntoResponse, AppError> {
    let comment = entry.comment.take();
    let filter = EventFilter::from(&entry);
    let mut tx = context.configdb.pool.begin().await?;

    let key = AutoArchive::key(&filter).unwrap();

    if let Ok(filters) = context.auto_archive.read()
        && filters.has_key(&key)
    {
        info!("Archive filters already contain key {}", &key);
        return Ok(Json(json!({})));
    }

    let sql = "INSERT INTO filters (user_id, filter, comment) VALUES (?, ?, ?)";
    sqlx::query(sql)
        .bind(0)
        .bind(sqlx::types::Json(&filter))
        .bind(&comment)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let mut ingest = context.auto_archive.write().unwrap();
    ingest.add(&filter);

    info!(
        "New auto-archive filter added {:?} with comment: {:?}",
        &filter, &comment
    );

    Ok(Json(json!({})))
}

pub(super) async fn get_filters(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    let rows = context.configdb.get_filters().await?;
    Ok(Json(rows))
}

pub(super) async fn delete_filter(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(id): Path<u32>,
) -> Result<impl IntoResponse, AppError> {
    // Remove from database.
    let mut tx = context.configdb.pool.begin().await?;
    let row: Option<FilterRow> =
        sqlx::query_as::<_, FilterRow>("SELECT * FROM filters WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if row.is_some()
        && sqlx::query("DELETE FROM filters WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .is_ok()
    {
        tx.commit().await?;
    }

    // Remove from current ingest processing.
    if let Some(row) = row {
        let mut ingest = context.auto_archive.write().unwrap();
        ingest.remove(&row.filter.0);
    }

    Ok(Json(json!({})))
}

/// One row in the `GET /api/agents/keys` listing: everything but the
/// key value itself, which stays reveal-on-demand.
#[derive(Debug, Serialize)]
pub(crate) struct AgentKeyInfo {
    pub id: i64,
    pub name: String,
    pub created_at: crate::datetime::ChronoDateTime,
    pub last_seen: Option<crate::datetime::ChronoDateTime>,
}

impl From<AgentKey> for AgentKeyInfo {
    fn from(row: AgentKey) -> Self {
        Self {
            id: row.id,
            name: row.name,
            created_at: row.created_at,
            last_seen: row.last_seen,
        }
    }
}

fn no_agent_key(id: i64) -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": format!("no agent key with id {id}")})),
    )
        .into_response()
}

/// `GET /api/agents/keys`: list agent keys without their key values.
pub(super) async fn get_agent_keys(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    let rows: Vec<AgentKeyInfo> = context
        .configdb
        .list_agent_keys()
        .await?
        .into_iter()
        .map(AgentKeyInfo::from)
        .collect();
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddAgentKeyRequest {
    pub name: String,
}

/// `POST /api/agents/keys`: create an agent key. The response is the
/// full row including the key value for first-time provisioning; the key
/// stays re-showable later through the reveal endpoint.
pub(super) async fn add_agent_key(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Json(request): Json<AddAgentKeyRequest>,
) -> Result<impl IntoResponse, AppError> {
    let added = context.configdb.add_agent_key(&request.name).await?;
    info!("Agent key {:?} added from the admin API", added.name);
    Ok(Json(added))
}

/// `GET /api/agents/keys/{id}`: the full row including the key value.
/// This is the reveal-on-demand endpoint; the listing never carries keys.
pub(super) async fn get_agent_key(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    match context.configdb.get_agent_key_by_id(id).await? {
        Some(row) => Ok(Json(row).into_response()),
        None => Ok(no_agent_key(id)),
    }
}

/// `DELETE /api/agents/keys/{id}`: remove an agent key and disconnect any
/// agent currently authenticated with it. The agent's reconnect attempt will
/// then fail because the key no longer exists.
pub(super) async fn delete_agent_key(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    match context.configdb.get_agent_key_by_id(id).await? {
        Some(row) => {
            context.configdb.remove_agent_key_by_id(id).await?;
            let disconnected = context.agents.revoke_key(id);
            info!(
                "Agent key {:?} removed from the admin API (connected agent bumped: {disconnected})",
                row.name
            );
            Ok(Json(json!({})).into_response())
        }
        None => Ok(no_agent_key(id)),
    }
}

pub(super) async fn kv_get_config(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    let rows = context.configdb.kv_get_config().await?;
    Ok(Json(rows))
}

pub(super) async fn kv_set_config(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(key): Path<String>,
    Json(value): Json<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    // The pcap routing table shares this kv namespace but has its own
    // endpoint that validates the table, applies it to the live
    // service, and serializes saves; a raw write here would bypass all
    // three and silently diverge the stored and live tables.
    if key == "config.pcap.routing" {
        return Err(AppError::BadRequest(
            "use /api/pcap/routing to modify the pcap routing table".to_string(),
        ));
    }
    context.configdb.kv_set_config(&key, &value).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use tokio::sync::Mutex;

    use crate::eventrepo::EventRepo;
    use crate::server::ServerConfig;
    use crate::server::main::build_axum_service;
    use crate::server::metrics::Metrics;
    use crate::sqlite::connection::{ConnectionBuilder, init_event_db};
    use crate::sqlite::eventrepo::SqliteEventRepo;

    use super::*;

    async fn serve_test_server() -> (
        std::net::SocketAddr,
        tokio::task::JoinHandle<()>,
        tempfile::TempDir,
        Arc<ServerContext>,
    ) {
        serve_test_server_with_config(ServerConfig::default()).await
    }

    async fn serve_test_server_with_config(
        config: ServerConfig,
    ) -> (
        std::net::SocketAddr,
        tokio::task::JoinHandle<()>,
        tempfile::TempDir,
        Arc<ServerContext>,
    ) {
        // The reqwest test client requires a process-wide TLS provider.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let dir = tempfile::tempdir().unwrap();
        let builder = ConnectionBuilder::filename(Some(&dir.path().join("events.sqlite")));
        let mut writer = builder.open_connection(true).await.unwrap();
        init_event_db(&mut writer).await.unwrap();
        let pool = builder.open_pool(false).await.unwrap();
        let datastore = EventRepo::SQLite(SqliteEventRepo::new(
            Arc::new(Mutex::new(writer)),
            pool,
            Arc::new(Metrics::default()),
        ));
        let configdb = crate::sqlite::configdb::open(Some(&dir.path().join("config.sqlite")))
            .await
            .unwrap();
        let context = Arc::new(ServerContext::new(
            config,
            Arc::new(configdb),
            datastore,
            Arc::new(Metrics::default()),
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let service = build_axum_service(context.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, service).await.unwrap();
        });
        (address, server, dir, context)
    }

    #[tokio::test]
    async fn agent_key_endpoints_manage_the_key_lifecycle() {
        let (address, server, _dir, context) = serve_test_server().await;
        let client = reqwest::Client::new();
        let collection = format!("http://{address}/api/agents/keys");

        let rows: Vec<Value> = client
            .get(&collection)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(rows.is_empty());

        let response = client
            .post(&collection)
            .json(&json!({"name": "sensor-a"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let created: Value = response.json().await.unwrap();
        assert_eq!(created["name"], "sensor-a");
        let key = created["key"].as_str().unwrap().to_string();
        assert!(key.starts_with(crate::sqlite::configdb::AGENT_KEY_PREFIX));
        let id = created["id"].as_i64().unwrap();

        // Model a live connection authenticated with the key so deletion can
        // prove it bumps the agent as well as removing the database row.
        let (outbound, _outbound_rx) =
            tokio::sync::mpsc::channel(crate::server::agents::OUTBOUND_CAPACITY);
        let live = context
            .agents
            .register(
                crate::agent::protocol::AgentHandshake {
                    name: "sensor-a".to_string(),
                    hostname: "sensor-a.example.test".to_string(),
                    version: "0.27.0-dev".to_string(),
                    capabilities: vec![crate::agent::protocol::CAPABILITY_PCAP.to_string()],
                },
                Some(crate::server::agents::AgentKeyIdentity {
                    id,
                    name: "sensor-a".to_string(),
                }),
                "127.0.0.1:12345".parse().unwrap(),
                outbound,
            )
            .unwrap();

        // Names that cannot be keyed: already in use, empty, reserved.
        for name in [
            "sensor-a".to_string(),
            "  ".to_string(),
            crate::server::agents::LOCAL_PCAP_SOURCE_NAME.to_string(),
            "x".repeat(crate::server::agents::MAX_AGENT_NAME_BYTES + 1),
        ] {
            let response = client
                .post(&collection)
                .json(&json!({"name": &name}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 400, "expected 400 for name {name:?}");
        }

        // The listing carries the metadata but never key values.
        let rows: Vec<Value> = client
            .get(&collection)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], id);
        assert_eq!(rows[0]["name"], "sensor-a");
        assert!(rows[0]["created_at"].is_string());
        assert!(rows[0]["last_seen"].is_null());
        assert!(rows[0].get("key").is_none());

        // Reveal re-shows the stored key.
        let item = format!("{collection}/{id}");
        let revealed: Value = client
            .get(&item)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(revealed["key"], key.as_str());

        // Delete, then every trace of the key is gone.
        assert_eq!(client.delete(&item).send().await.unwrap().status(), 200);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), live.cancelled())
                .await
                .is_ok()
        );
        assert_eq!(client.get(&item).send().await.unwrap().status(), 404);
        assert_eq!(client.delete(&item).send().await.unwrap().status(), 404);
        let rows: Vec<Value> = client
            .get(&collection)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(rows.is_empty());

        server.abort();
    }

    /// The key endpoints carry live credentials, so pin their auth
    /// posture: with authentication required, every method rejects a
    /// session-less request. Losing a SessionExtractor parameter from a
    /// handler would otherwise still compile and pass the lifecycle test.
    #[tokio::test]
    async fn agent_key_endpoints_require_authentication() {
        let config = ServerConfig {
            authentication_required: true,
            ..ServerConfig::default()
        };
        let (address, server, _dir, _context) = serve_test_server_with_config(config).await;
        let client = reqwest::Client::new();
        let collection = format!("http://{address}/api/agents/keys");
        let item = format!("{collection}/1");

        assert_eq!(client.get(&collection).send().await.unwrap().status(), 401);
        assert_eq!(
            client
                .post(&collection)
                .json(&json!({"name": "sensor-a"}))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
        assert_eq!(client.get(&item).send().await.unwrap().status(), 401);
        assert_eq!(client.delete(&item).send().await.unwrap().status(), 401);

        server.abort();
    }

    /// The pcap routing endpoints: a save validates, trims, persists,
    /// and applies the table live — the GET reads the running service,
    /// not the database — and the kv row survives for the next server
    /// start.
    #[tokio::test]
    async fn pcap_routing_endpoints_roundtrip() {
        let (address, server, dir, _context) = serve_test_server().await;
        let client = reqwest::Client::new();
        let url = format!("http://{address}/api/pcap/routing");

        // Absent table: empty rules, no default.
        let table: Value = client.get(&url).send().await.unwrap().json().await.unwrap();
        assert_eq!(table, json!({"rules": [], "default": null}));

        let response = client
            .post(&url)
            .json(&json!({
                "rules": [{"sensor": " sensor-1 ", "source": " fw-east "}],
                "default": " (server) ",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let table: Value = client.get(&url).send().await.unwrap().json().await.unwrap();
        assert_eq!(
            table,
            json!({
                "rules": [{"sensor": "sensor-1", "source": "fw-east"}],
                "default": "(server)",
            })
        );

        // The table reached the configdb, not just the live service.
        let configdb = crate::sqlite::configdb::open(Some(&dir.path().join("config.sqlite")))
            .await
            .unwrap();
        let persisted: crate::server::pcap::PcapRouting = configdb
            .kv_get_config_as_t("config.pcap.routing")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(persisted.default.as_deref(), Some("(server)"));
        assert_eq!(persisted.rules.len(), 1);

        // Invalid tables are refused and change nothing: empty names,
        // and tables over the rule-count or name-length caps.
        let many: Vec<Value> = (0..257)
            .map(|i| json!({"sensor": format!("s-{i}"), "source": "x"}))
            .collect();
        for bad in [
            json!({"rules": [{"sensor": "", "source": "fw-east"}], "default": null}),
            json!({"rules": [{"sensor": "sensor-1", "source": " "}], "default": null}),
            json!({"rules": [], "default": "  "}),
            json!({"rules": many, "default": null}),
            json!({"rules": [{"sensor": "x".repeat(crate::server::agents::MAX_AGENT_NAME_BYTES + 1), "source": "fw-east"}], "default": null}),
            json!({"rules": [], "default": "x".repeat(crate::server::agents::MAX_AGENT_NAME_BYTES + 1)}),
        ] {
            let response = client.post(&url).json(&bad).send().await.unwrap();
            assert_eq!(response.status(), 400, "expected 400 for {bad}");
        }
        let table: Value = client.get(&url).send().await.unwrap().json().await.unwrap();
        assert_eq!(table["rules"][0]["sensor"], "sensor-1");

        server.abort();
    }

    /// A raw kv write to the routing key would bypass validation, the
    /// live apply, and the save lock, so the generic config endpoint
    /// refuses it while other keys keep working.
    #[tokio::test]
    async fn kv_config_endpoint_refuses_the_pcap_routing_key() {
        let (address, server, _dir, _context) = serve_test_server().await;
        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "http://{address}/api/admin/kv/config/config.pcap.routing"
            ))
            .json(&json!({"rules": "x"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        let response = client
            .post(format!(
                "http://{address}/api/admin/kv/config/config.retention"
            ))
            .json(&json!({"days": 7}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        server.abort();
    }

    /// The routing table controls which source serves packet captures,
    /// so pin its auth posture like the key endpoints above.
    #[tokio::test]
    async fn pcap_routing_endpoints_require_authentication() {
        let config = ServerConfig {
            authentication_required: true,
            ..ServerConfig::default()
        };
        let (address, server, _dir, _context) = serve_test_server_with_config(config).await;
        let client = reqwest::Client::new();
        let url = format!("http://{address}/api/pcap/routing");

        assert_eq!(client.get(&url).send().await.unwrap().status(), 401);
        assert_eq!(
            client
                .post(&url)
                .json(&json!({"rules": [], "default": "fw-east"}))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );

        server.abort();
    }
}
