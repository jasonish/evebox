// Copyright (C) 2020-2021 Jason Ish
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

use std::sync::Arc;

use serde::Serialize;

pub(crate) use main::build_axum_server;
pub use main::build_context;
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

#[derive(Debug, Clone, PartialEq)]
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
    pub datastore: Datastore,
    pub features: Features,
    pub session_store: session::SessionStore,
    pub config_repo: Arc<ConfigRepo>,
    pub event_services: Option<serde_json::Value>,
}

impl ServerContext {
    pub fn new(config: ServerConfig, config_repo: Arc<ConfigRepo>, datastore: Datastore) -> Self {
        Self {
            config: config,
            datastore,
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
