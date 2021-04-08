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

// Compiler suppressions.
#![allow(dead_code)]
// #![allow(unused_imports)]

// Clippy suppressions.
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::needless_update)]
#![allow(clippy::implicit_hasher)]
#![allow(clippy::module_inception)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::new_without_default)]

pub mod agent;
pub mod bookmark;
pub mod commands;
mod datastore;
mod elastic;
pub mod eve;
pub mod geoip;
pub mod importer;
pub mod logger;
pub mod packet;
mod path;
pub mod pcap;
pub mod prelude;
pub mod resource;
mod rules;
pub mod server;
pub mod settings;
pub mod sqlite;
pub mod types;
pub mod version;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate serde_json;
