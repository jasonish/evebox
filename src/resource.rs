// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use rust_embed::{EmbeddedFile, RustEmbed};

#[derive(RustEmbed)]
#[folder = "./resources"]
pub struct Resource;

pub fn get(file_path: &str) -> Option<EmbeddedFile> {
    Resource::get(file_path)
}

pub fn get_string(file_path: &str) -> Option<String> {
    if let Some(bytes) = get(file_path) {
        if let Ok(text) = std::str::from_utf8(&bytes.data) {
            return Some(text.to_string());
        }
    }
    None
}
