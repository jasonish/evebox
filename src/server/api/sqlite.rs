// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    eventrepo::EventRepo,
    server::{main::SessionExtractor, ServerContext},
    sqlite::{self, info::Info},
};
use axum::{response::IntoResponse, Extension, Json};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;

use super::ApiError;

pub(crate) async fn info(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<impl IntoResponse, ApiError> {
    if let EventRepo::SQLite(sqlite) = &context.datastore {
        #[derive(Default, Serialize)]
        struct Response {
            auto_vacuum: u8,
            journal_mode: String,
            synchronous: u8,
            fts_enabled: bool,
            page_size: i64,
            page_count: i64,
            freelist_count: i64,
            min_event_id: u64,
            max_event_id: u64,
            event_count_estimate: u64,
            data_size: u64,
            schema_version: u64,
            min_timestamp: Option<String>,
            max_timestamp: Option<String>,
        }

        let min_row_id = sqlite.min_row_id().await?;
        let max_row_id = sqlite.max_row_id().await?;
        let event_count_estimate = max_row_id - min_row_id;

        let min_timestamp = sqlite.min_timestamp().await?;
        let max_timestamp = sqlite.max_timestamp().await?;

        let response = sqlite
            .pool
            .get()
            .await?
            .interact(move |conn| -> Result<Response, rusqlite::Error> {
                let mut response = Response {
                    min_timestamp: min_timestamp.map(|ts| ts.to_string()),
                    max_timestamp: max_timestamp.map(|ts| ts.to_string()),
                    ..Default::default()
                };

                let info = Info::new(conn);
                response.auto_vacuum = info.get_auto_vacuum()?;
                response.journal_mode = info.get_journal_mode()?;
                response.synchronous = info.get_synchronous()?;
                response.fts_enabled = info.has_table("fts")?;
                response.page_size = info.get_pragma::<i64>("page_size")?;
                response.page_count = info.get_pragma::<i64>("page_count")?;
                response.freelist_count = info.get_pragma::<i64>("freelist_count")?;
                response.min_event_id = min_row_id;
                response.max_event_id = max_row_id;
                response.data_size = (response.page_size * response.page_count) as u64;
                response.schema_version = info.schema_version()?;
                response.event_count_estimate = event_count_estimate;
                Ok(response)
            })
            .await??;
        return Ok(Json(response).into_response());
    }

    Ok(().into_response())
}

pub(crate) async fn fts_check(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<impl IntoResponse, ApiError> {
    if let EventRepo::SQLite(sqlite) = &context.datastore {
        #[derive(Debug, Serialize)]
        struct Response {
            ok: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<String>,
        }

        info!("Running SQLite FTS integrity check from API");

        let response = sqlite
            .pool
            .get()
            .await?
            .interact(move |conn| -> Result<Response, rusqlite::Error> {
                let response = match sqlite::util::fts_check(conn) {
                    Ok(_) => Response {
                        ok: true,
                        error: None,
                    },
                    Err(err) => Response {
                        ok: false,
                        error: Some(format!("{:?}", err)),
                    },
                };

                Ok(response)
            })
            .await??;
        return Ok(Json(response).into_response());
    }

    Ok(().into_response())
}
