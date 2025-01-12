// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::eventrepo::EventRepo;
use crate::sqlite::configdb::ConfigDb;
pub(crate) use main::build_context;
pub use main::main;
use serde::Serialize;
use session::SessionStore;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub(crate) mod api;
pub(crate) mod main;
pub(crate) mod session;

#[derive(Serialize, Default, Debug)]
pub(crate) struct Defaults {
    pub time_range: Option<String>,
}

pub(crate) struct ServerContext {
    pub config: ServerConfig,
    pub datastore: EventRepo,
    pub session_store: session::SessionStore,
    pub configdb: Arc<ConfigDb>,
    pub event_services: Option<serde_json::Value>,
    pub defaults: Defaults,
    pub ingest: crate::ingest::IngestPipeline,
    pub auto_archive: Arc<RwLock<crate::ingest::AutoArchive>>,
}

impl ServerContext {
    pub(crate) fn new(
        config: ServerConfig,
        config_repo: Arc<ConfigDb>,
        datastore: EventRepo,
    ) -> Self {
        let auto_archive: Arc<RwLock<crate::ingest::AutoArchive>> = Default::default();
        Self {
            config,
            datastore,
            session_store: SessionStore::new(),
            configdb: config_repo,
            event_services: None,
            defaults: Defaults::default(),
            ingest: crate::ingest::IngestPipeline::new(auto_archive.clone()),
            auto_archive,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub no_check_certificate: bool,
    pub datastore: String,
    pub tls_enabled: bool,
    pub tls_cert_filename: Option<PathBuf>,
    pub tls_key_filename: Option<PathBuf>,
    pub elastic_url: String,
    pub elastic_index: String,
    pub elastic_no_index_suffix: bool,
    pub elastic_username: Option<String>,
    pub elastic_password: Option<String>,
    pub elastic_ecs: bool,
    pub data_directory: Option<String>,
    pub authentication_required: bool,
    pub http_reverse_proxy: bool,
    pub http_request_logging: bool,
}
