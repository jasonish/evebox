// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::error::AppError;
use crate::prelude::*;
use crate::server::{ServerContext, main::SessionExtractor};
use axum::Extension;
use axum::body::Body;
use axum::response::sse::Event;
use axum::response::{IntoResponse, Sse};
use futures::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub(crate) async fn sse(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let mut firehose = context.firehose.subscribe();

    let (sse_tx, sse_rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, Infallible>>();
    let sse_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(sse_rx);

    tokio::spawn(async move {
        while let Ok(event) = firehose.recv().await {
            let event = match Event::default().json_data(event) {
                Ok(event) => event,
                Err(_) => return,
            };
            if sse_tx.send(Ok(event)).is_err() {
                warn!("Client disappeared, terminating SSE firehose");
                return;
            }
        }
    });

    Ok(Sse::new(sse_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    ))
}

pub(crate) async fn stream(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> impl IntoResponse {
    let firehose: tokio::sync::broadcast::Receiver<serde_json::Value> =
        context.firehose.subscribe();

    let stream = BroadcastStream::new(firehose).filter_map(|result| match result {
        Ok(value) => {
            let mut string = value.to_string();
            string.push('\n');
            let bytes = axum::body::Bytes::from(string);
            Some(Ok::<bytes::Bytes, BroadcastStreamRecvError>(bytes))
        }
        Err(_) => None,
    });

    axum::response::Response::builder()
        .body(Body::from_stream(stream))
        .unwrap()
}
