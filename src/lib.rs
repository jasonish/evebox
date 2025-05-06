// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub mod cli;
pub mod logger;
pub mod server;
pub mod version;

mod agent;
mod bookmark;
mod cert;
mod commands;
mod config;
mod datetime;
mod elastic;
mod error;
mod eve;
mod eventrepo;
mod file;
mod geoip;
mod importer;
mod path;
mod prelude;
mod queryparser;
mod resource;
mod rules;
mod sqlite;
mod util;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate serde_json;

lazy_static! {
    /// Environment variable to enable query logging.
    static ref LOG_QUERIES: bool = std::env::var("EVEBOX_LOG_QUERIES").is_ok();

    /// Environment variable to enable logging of query plans.
    static ref LOG_QUERY_PLAN: bool = std::env::var("EVEBOX_LOG_QUERY_PLAN").is_ok();
}
