// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::sync::{OnceLock, RwLock};

use super::ConfigDb;

pub static CONFIGDB: OnceLock<RwLock<Option<ConfigDb>>> = OnceLock::new();

pub(crate) fn set_configdb(configdb: ConfigDb) {
    CONFIGDB.get_or_init(|| RwLock::new(Some(configdb)));
}

pub(crate) fn get_configdb() -> Option<ConfigDb> {
    CONFIGDB
        .get_or_init(|| RwLock::new(None))
        .read()
        .unwrap()
        .clone()
}
