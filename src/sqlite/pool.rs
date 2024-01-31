// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use std::path::PathBuf;

pub async fn open_pool<T: Into<PathBuf>>(filename: T) -> Result<deadpool_sqlite::Pool> {
    use deadpool_sqlite::{Config, Runtime};
    let config = Config::new(filename);
    let pool = config.create_pool(Runtime::Tokio1)?;
    Ok(pool)
}
