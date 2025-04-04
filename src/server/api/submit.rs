// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::server::ServerContext;
use axum::body::Bytes;
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::io::BufRead;
use std::sync::Arc;
use tracing::error;

pub(crate) async fn handler(
    Extension(context): Extension<Arc<ServerContext>>,
    body: Bytes,
) -> impl IntoResponse {
    let mut importer = match context.datastore.get_importer() {
        Some(importer) => importer,
        None => {
            return (StatusCode::NOT_IMPLEMENTED, "").into_response();
        }
    };
    let mut errors = Vec::new();

    let mut buf = &body[..];
    let mut count = 0;
    let mut line = String::new();
    loop {
        match buf.read_line(&mut line) {
            Err(err) => {
                errors.push(format!("Failed to read event from request body: {err}"));
                // Failed to read line, can't continue.
                break;
            }
            Ok(n) => {
                if n == 0 {
                    // EOF.
                    break;
                }
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Err(err) => {
                        errors.push(format!(
                            "Failed to decode event from request body ({err}): {line}"
                        ));
                    }
                    Ok(mut event) => {
                        count += 1;

                        if let Some(filters) = &context.filters {
                            filters.run(&mut event);
                        }

                        if let Err(err) = importer.submit(event.clone()).await {
                            error!("Failed to submit event to importer: {}", err);
                        }

                        let _ = context.firehose.send(event);
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
        return Json(response).into_response();
    }

    match importer.commit().await {
        Ok(n) => {
            context.metrics.incr_events_rx(count);
            let response = json!({
                // Kept capitolized for compatibility with the Go agent.
                "Count": n,
            });
            Json(response).into_response()
        }
        Err(err) => {
            error!("Failed to commit events (received {}): {:#}", count, err);
            (StatusCode::INTERNAL_SERVER_ERROR, "").into_response()
        }
    }
}
