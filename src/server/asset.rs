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
