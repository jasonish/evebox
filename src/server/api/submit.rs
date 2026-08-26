// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::server::ServerContext;
use axum::Json;
use axum::body::Bytes;
use axum::extract::Extension;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::json;
use std::io::BufRead;
use std::sync::Arc;
use tracing::error;

use crate::agent::protocol::AGENT_KEY_HEADER;

pub(crate) async fn handler(
    Extension(context): Extension<Arc<ServerContext>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // Event submission remains open for legacy agents. When an agent key is
    // presented, however, it is authoritative: validate it and use its name
    // as the non-secret routing identity stamped into every submitted event.
    // Without a key no stamp is trusted: `evebox.agent.id` routes packet
    // capture requests to a named agent's spool, so a client-supplied value
    // is removed and such events fall back to hostname routing.
    let presented_key = match headers.get(AGENT_KEY_HEADER) {
        Some(value) => match value.to_str() {
            Ok(value) => Some(value.to_string()),
            Err(_) => return (StatusCode::UNAUTHORIZED, "invalid agent key").into_response(),
        },
        None => None,
    };
    let agent_name = if let Some(key) = presented_key {
        match context.configdb.verify_agent_key(&key).await {
            Ok(Some(key)) => Some(key.name),
            Ok(None) => {
                return (StatusCode::UNAUTHORIZED, "unknown agent key").into_response();
            }
            Err(err) => {
                error!("Agent key verification failed during event submission: {err}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "agent key verification failed",
                )
                    .into_response();
            }
        }
    } else {
        None
    };

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
                        match &agent_name {
                            Some(agent_name) if event.is_object() => {
                                event["evebox"]["agent"]["id"] = agent_name.clone().into();
                            }
                            Some(_) => {}
                            None => {
                                if let Some(agent) = event
                                    .get_mut("evebox")
                                    .and_then(|evebox| evebox.get_mut("agent"))
                                    .and_then(|agent| agent.as_object_mut())
                                {
                                    agent.remove("id");
                                }
                            }
                        }

                        if let Err(err) = importer.submit(event.clone()).await {
                            error!("Failed to submit event to importer: {}", err);
                        }

                        let _ = context.firehose.send(event);
                    }
                }
            }
        }
        line.clear();
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
