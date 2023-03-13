// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::eventrepo::EventRepo;
use crate::sqlite::configrepo::ConfigRepo;
pub(crate) use main::build_axum_server;
pub use main::build_context;
pub use main::main;
use serde::Serialize;
use session::SessionStore;
use std::sync::Arc;

pub mod api;
mod asset;
mod main;
mod rejection;
mod response;
pub mod session;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticationType {
    Anonymous,
    Username,
    UsernamePassword,
}

impl ToString for AuthenticationType {
    fn to_string(&self) -> String {
        let s = match self {
            AuthenticationType::Anonymous => "anonymous",
            AuthenticationType::Username => "username",
            AuthenticationType::UsernamePassword => "usernamepassword",
        };
        s.to_string()
    }
}

impl Default for AuthenticationType {
    fn default() -> Self {
        Self::Anonymous
    }
}

#[derive(Serialize, Default, Debug)]
pub struct Features {
    pub comments: bool,
    pub reporting: bool,
}

#[derive(Serialize, Default, Debug)]
pub struct Defaults {
    pub time_range: Option<String>,
}

pub struct ServerContext {
    pub config: ServerConfig,
    pub datastore: EventRepo,
    pub features: Features,
    pub session_store: session::SessionStore,
    pub config_repo: Arc<ConfigRepo>,
    pub event_services: Option<serde_json::Value>,
    pub defaults: Defaults,
}

impl ServerContext {
    pub fn new(config: ServerConfig, config_repo: Arc<ConfigRepo>, datastore: EventRepo) -> Self {
        Self {
            config,
            datastore,
            features: Features::default(),
            session_store: SessionStore::new(),
            config_repo,
            event_services: None,
            defaults: Defaults::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub no_check_certificate: bool,
    pub datastore: String,
    pub sqlite_filename: Option<String>,
    pub tls_enabled: bool,
    pub tls_cert_filename: Option<String>,
    pub tls_key_filename: Option<String>,
    pub elastic_url: String,
    pub elastic_index: String,
    pub elastic_no_index_suffix: bool,
    pub elastic_username: Option<String>,
    pub elastic_password: Option<String>,
    pub elastic_ecs: bool,
    pub data_directory: Option<String>,
    pub authentication_required: bool,
    pub authentication_type: AuthenticationType,
    pub http_reverse_proxy: bool,
    pub http_request_logging: bool,
}
