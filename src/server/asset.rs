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

use crate::logger::log;
use warp::http::StatusCode;

pub fn new_static_or_404(path: &str) -> Box<dyn warp::Reply> {
    log::debug!("Loading asset {}", path);
    let path = format!("public/{}", path);
    let asset = crate::resource::Resource::get(&path);
    if let Some(asset) = asset {
        let content_type = {
            if path.ends_with(".html") {
                "text/html"
            } else if path.ends_with(".js") {
                "application/javascript"
            } else if path.ends_with(".css") {
                "text/css"
            } else if path.ends_with(".ico") {
                "image/x-icon"
            } else {
                "application/octet-stream"
            }
        };
        let reply =
            warp::reply::with_header(asset.into_owned(), "content-type", content_type.to_string());
        return Box::new(reply);
    }

    log::warn!("Failed to find static asset: {}", path);
    Box::new(warp::reply::with_status("", StatusCode::NOT_FOUND))
}
