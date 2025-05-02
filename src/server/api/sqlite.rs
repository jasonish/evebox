// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    eventrepo::EventRepo,
    server::{main::SessionExtractor, ServerContext},
    sqlite::{self, info::Info},
};
use axum::{response::IntoResponse, Extension, Json};
use serde::Serialize;
use sqlx::Connection;
use std::sync::Arc;
use tracing::info;

use crate::error::AppError;

pub(crate) async fn info(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<impl IntoResponse, AppError> {
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
            schema_version: i64,
            min_timestamp: Option<String>,
            max_timestamp: Option<String>,
        }

        let min_row_id = sqlite.min_row_id().await?;
        let max_row_id = sqlite.max_row_id().await?;
        let event_count_estimate = max_row_id - min_row_id;

        let min_timestamp = sqlite.earliest_timestamp().await?;
        let max_timestamp = sqlite.max_timestamp().await?;

        let mut response = Response {
            min_timestamp: min_timestamp.map(|ts| ts.to_string()),
            max_timestamp: max_timestamp.map(|ts| ts.to_string()),
            min_event_id: min_row_id,
            max_event_id: max_row_id,
            event_count_estimate,
            ..Default::default()
        };

        let mut tx = sqlite.pool.begin().await?;
        let mut info = Info::new(&mut tx);
        response.auto_vacuum = info.get_auto_vacuum().await?;
        response.journal_mode = info.get_journal_mode().await?;
        response.synchronous = info.get_synchronous().await?;
        response.fts_enabled = info.has_table("fts").await?;
        response.page_size = info.pragma_i64("page_size").await?;
        response.page_count = info.pragma_i64("page_count").await?;
        response.freelist_count = info.pragma_i64("freelist_count").await?;
        response.data_size = (response.page_size * response.page_count) as u64;
        response.schema_version = info.schema_version().await?;

        return Ok(Json(response).into_response());
    }

    Ok(().into_response())
}

pub(crate) async fn fts_check(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<impl IntoResponse, AppError> {
    if let EventRepo::SQLite(sqlite) = &context.datastore {
        #[derive(Debug, Serialize)]
        struct Response {
            ok: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<String>,
        }

        info!("Running SQLite FTS integrity check from API");

        let mut tx = sqlite.pool.begin().await?;
        let response = match sqlite::util::fts_check(&mut tx).await {
            Ok(_) => Response {
                ok: true,
                error: None,
            },
            Err(err) => Response {
                ok: false,
                error: Some(format!("{err:?}")),
            },
        };

        return Ok(Json(response).into_response());
    }

    Ok(().into_response())
}

pub(crate) async fn fts_enable(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<impl IntoResponse, AppError> {
    if let EventRepo::SQLite(sqlite) = &context.datastore {
        info!("Enabling SQLite FTS from API");
        let mut conn = sqlite.writer.lock().await;
        let mut tx = conn.begin().await?;
        crate::sqlite::util::fts_enable(&mut tx).await?;
        tx.commit().await?;
    }

    Ok(().into_response())
}

pub(crate) async fn fts_disable(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<impl IntoResponse, AppError> {
    if let EventRepo::SQLite(sqlite) = &context.datastore {
        info!("Disabling SQLite FTS from API");
        let mut conn = sqlite.writer.lock().await;
        let mut tx = conn.begin().await?;
        crate::sqlite::util::fts_disable(&mut tx).await?;
        tx.commit().await?;
    }

    Ok(().into_response())
}
