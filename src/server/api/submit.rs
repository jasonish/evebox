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

use std::io::BufRead;
use std::sync::Arc;

use crate::eve::eve::EveJson;
use crate::logger::log;
use crate::server::response::Response;
use crate::server::ServerContext;

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
    body: bytes::Bytes,
) -> Result<impl warp::Reply, warp::Rejection> {
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
                            log::error!("Failed to submit event to importer: {}", err);
                        }
                    }
                }
            }
        }
        line.truncate(0);
    }
    match importer.commit().await {
        Ok(n) => {
            log::debug!("Committed {} events (received {})", n, count);
            return Ok(Response::Ok);
        }
        Err(err) => {
            log::error!("Failed to commit events (received {}): {}", count, err);
            return Ok(Response::InternalError(err.to_string()));
        }
    }
}
