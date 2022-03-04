// Copyright (C) 2020 Jason Ish
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

// Clippy suppressions. These are the global ones I don't care about.
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_field_names)]

#[macro_use]
pub mod logger;

pub mod agent;
pub mod bookmark;
pub mod commands;
pub mod config;
mod datastore;
mod elastic;
pub mod eve;
pub mod geoip;
pub mod importer;
pub mod oldconfig;
pub mod oldsettings;
pub mod packet;
mod path;
pub mod pcap;
pub mod prelude;
pub mod resource;
mod rules;
pub mod searchquery;
pub mod server;
pub mod sqlite;
pub mod types;
pub mod version;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate serde_json;
