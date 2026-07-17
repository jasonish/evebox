// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::eventrepo::EventRepo;
use crate::server::autoarchive::AutoArchive;
use crate::sqlite::configdb::ConfigDb;
pub(crate) use main::build_context;
pub use main::main;
use metrics::Metrics;
use serde::Serialize;
use session::SessionStore;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub(crate) mod agents;
pub(crate) mod api;
pub(crate) mod autoarchive;
pub(crate) mod context;
pub(crate) mod main;
pub(super) mod metrics;
pub(crate) mod pcap;
pub(crate) mod session;

const SUPPORTED_DEFAULT_TIME_RANGES: [&str; 9] =
    ["1m", "1h", "3h", "12h", "24h", "1d", "3d", "7d", "all"];

pub(crate) fn parse_default_time_range(value: &str) -> Result<String, String> {
    if SUPPORTED_DEFAULT_TIME_RANGES.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(format!(
            "unsupported time range '{value}'; expected one of: {}",
            SUPPORTED_DEFAULT_TIME_RANGES.join(", ")
        ))
    }
}

#[derive(Serialize, Default, Debug)]
pub(crate) struct Defaults {
    pub time_range: Option<String>,
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ServerMode {
    #[default]
    Server,
    Oneshot,
}

pub(crate) struct ServerContext {
    pub config: ServerConfig,
    pub mode: ServerMode,
    pub datastore: EventRepo,
    pub session_store: session::SessionStore,
    pub configdb: Arc<ConfigDb>,
    pub event_services: Option<serde_json::Value>,
    pub defaults: Defaults,
    pub filters: Option<crate::eve::filters::EveFilterChain>,
    pub auto_archive: Arc<RwLock<AutoArchive>>,
    pub metrics: Arc<Metrics>,
    pub firehose: tokio::sync::broadcast::Sender<serde_json::Value>,
    pub(crate) agents: Arc<agents::AgentRegistry>,
    pub(crate) pcap_tasks: Arc<pcap::tasks::Registry>,
    pub pcap: Arc<pcap::PcapService>,
}

impl ServerContext {
    pub(crate) fn new(
        config: ServerConfig,
        config_repo: Arc<ConfigDb>,
        datastore: EventRepo,
        metrics: Arc<Metrics>,
    ) -> Self {
        let (firehose, _) = tokio::sync::broadcast::channel::<serde_json::Value>(8192);
        let auto_archive: Arc<RwLock<AutoArchive>> = Default::default();
        let pcap_tasks = Arc::new(pcap::tasks::Registry::default());
        let agents = Arc::new(agents::AgentRegistry::new(pcap_tasks.clone()));
        Self {
            config,
            mode: ServerMode::default(),
            datastore,
            session_store: SessionStore::new(),
            configdb: config_repo,
            event_services: None,
            defaults: Defaults::default(),
            filters: None,
            auto_archive,
            metrics,
            firehose,
            agents,
            pcap_tasks,
            pcap: Arc::new(pcap::PcapService::default()),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub no_check_certificate: bool,
    pub datastore: String,
    pub retention_disabled: bool,
    pub tls_enabled: bool,
    pub tls_cert_filename: Option<PathBuf>,
    pub tls_key_filename: Option<PathBuf>,
    pub elastic_url: String,
    pub elastic_index: String,
    pub elastic_no_index_suffix: bool,
    pub elastic_username: Option<String>,
    pub elastic_password: Option<String>,
    pub elastic_cacert: Option<String>,
    pub elastic_ecs: bool,
    pub data_directory: Option<String>,
    pub config_directory: Option<String>,
    pub authentication_required: bool,
    pub http_reverse_proxy: bool,
    pub http_request_logging: bool,
    /// Accept agent control-channel connections without an agent key. A lab
    /// escape hatch: agent keys are otherwise required regardless of
    /// `authentication.required`, which only governs browser access.
    pub agents_allow_unauthenticated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_default_time_ranges_are_accepted() {
        for value in SUPPORTED_DEFAULT_TIME_RANGES {
            assert_eq!(parse_default_time_range(value), Ok(value.to_string()));
        }
    }

    #[test]
    fn unsupported_default_time_ranges_are_rejected() {
        for value in ["", "60s", "6h", "30d", "invalid"] {
            assert!(parse_default_time_range(value).is_err(), "{value}");
        }
    }
}
