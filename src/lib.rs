// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

#[macro_use]
pub mod logger;

pub mod agent;
pub mod bookmark;
pub(crate) mod cert;
pub mod commands;
pub mod config;
mod elastic;
pub mod eve;
mod eventrepo;
pub(crate) mod file;
pub mod geoip;
pub mod importer;
pub mod packet;
mod path;
pub mod pcap;
pub mod prelude;
pub mod querystring;
pub mod resource;
mod rules;
pub mod server;
pub mod sqlite;
pub(crate) mod util;
pub mod version;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate serde_json;

lazy_static! {
    pub static ref LOG_QUERIES: bool = std::env::var_os("EVEBOX_LOG_QUERIES").is_some();
}
