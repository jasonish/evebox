// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::sync::Arc;

use serde::Serialize;

pub use main::build_context;
pub use main::build_server_try_bind;
pub use main::main;
use session::SessionStore;

use crate::datastore::Datastore;
use crate::sqlite::configrepo::ConfigRepo;

pub mod api;
mod asset;
mod filters;
mod main;
mod rejection;
mod response;
pub mod session;

#[derive(Debug, Clone)]
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

pub struct ServerContext {
    pub config: ServerConfig,
    pub elastic: Option<crate::elastic::EventStore>,
    pub datastore: Datastore,
    pub features: Features,
    pub session_store: session::SessionStore,
    pub config_repo: Arc<ConfigRepo>,
    pub event_services: Option<serde_json::Value>,
}

impl ServerContext {
    pub fn new(config: ServerConfig, config_repo: Arc<ConfigRepo>) -> Self {
        Self {
            config: config,
            elastic: None,
            datastore: Datastore::None,
            features: Features::default(),
            session_store: SessionStore::new(),
            config_repo: config_repo,
            event_services: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: String,
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
    pub database_retention_period: Option<u64>,
    pub http_reverse_proxy: bool,
    pub http_request_logging: bool,
}
