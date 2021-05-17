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

use crate::prelude::*;
use std::io::BufRead;
use std::sync::Arc;

use crate::eve::eve::EveJson;
use crate::server::response::Response;
use crate::server::ServerContext;
use std::convert::Infallible;

struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    fn next_record(&mut self) -> Result<Option<EveJson>, Box<dyn std::error::Error>> {
        let mut line = String::new();
        let n = self.buf.read_line(&mut line)?;
        if n == 0 {
            Ok(None)
        } else {
            let event: EveJson = serde_json::from_str(&line)?;
            Ok(Some(event))
        }
    }
}

pub async fn handler(
    context: Arc<ServerContext>,
    body: warp::hyper::body::Bytes,
) -> Result<impl warp::Reply, Infallible> {
    let mut importer = match context.datastore.get_importer() {
        Some(importer) => importer,
        None => {
            return Ok(crate::server::response::Response::Unimplemented);
        }
    };
    let mut errors = Vec::new();

    let mut buf = &body[..];
    let mut count = 0;
    let mut line = String::new();
    loop {
        match buf.read_line(&mut line) {
            Err(err) => {
                errors.push(format!("Failed to read event from request body: {}", err));
                // Failed to read line, can't continue.
                break;
            }
            Ok(n) => {
                if n == 0 {
                    // EOF.
                    break;
                }
                match serde_json::from_str::<EveJson>(&line) {
                    Err(err) => {
                        errors.push(format!(
                            "Failed to decode event from request body ({}): {}",
                            err, line
                        ));
                    }
                    Ok(event) => {
                        count += 1;
                        if let Err(err) = importer.submit(event).await {
                            error!("Failed to submit event to importer: {}", err);
                        }
                    }
                }
            }
        }
        line.truncate(0);
    }

    // I've seen an issue in the Go agent where it sent 0 events, return early if we have
    // nothing to commit.
    if count == 0 {
        // TODO: Log something or return an error to the client.
        let response = json!({
            "Count": 0,
        });
        return Ok(Response::Json(response));
    }

    match importer.commit().await {
        Ok(n) => {
            debug!("Committed {} events (received {})", n, count);
            let response = json!({
                // Kept capitolized for compatibility with the Go agent.
                "Count": n,
            });
            return Ok(Response::Json(response));
        }
        Err(err) => {
            error!("Failed to commit events (received {}): {}", count, err);
            return Ok(Response::InternalError(err.to_string()));
        }
    }
}
