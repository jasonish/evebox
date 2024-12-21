// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use axum::{Extension, Json};

use crate::prelude::*;
use crate::server::{main::SessionExtractor, ServerContext};

pub(super) async fn update_ja4db(
    Extension(context): Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("API request to update JA4 database");
    match do_update(context).await {
        Ok(response) => {
            info!("JA4db updated");
            Ok(response)
        }
        Err(err) => {
            error!("Request to update JA4db failed: {err}");
            Err(err.into())
        }
    }
}

async fn do_update(context: Arc<ServerContext>) -> Result<Json<serde_json::Value>> {
    let mut conn = context.config_repo.pool.begin().await?;
    let n = crate::commands::ja4db::updatedb(&mut conn).await?;
    conn.commit().await?;
    let response = json!({
        "entries": n,
    });
    Ok(Json(response))
}
