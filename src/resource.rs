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

use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "./resources"]
pub struct Resource;

pub fn get(file_path: &str) -> Option<Cow<'static, [u8]>> {
    Resource::get(file_path)
}

pub fn get_string(file_path: &str) -> Option<String> {
    if let Some(bytes) = get(file_path) {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            return Some(text.to_string());
        }
    }
    None
}
