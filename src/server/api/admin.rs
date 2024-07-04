// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use axum::{Extension, Json};
use tracing::info;

use crate::server::{main::SessionExtractor, ServerContext};

use super::ApiError;

pub(super) async fn update_ja4db(
    context: Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut conn = context.config_repo.pool.begin().await?;
    info!("Updating JA4db");
    let n = crate::commands::ja4db::updatedb(&mut conn).await?;
    conn.commit().await?;
    let response = json!({
        "entries": n,
    });
    info!("JA4db successfully updated: entries={n}");
    Ok(Json(response))
}
