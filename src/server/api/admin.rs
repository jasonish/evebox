// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use axum::response::IntoResponse;
use axum::Form;
use axum::{extract::Path, Extension, Json};

use crate::server::{main::SessionExtractor, ServerContext};
use crate::sqlite::configdb::{FilterEntry, FilterRow};

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
    let mut conn = context.configdb.pool.begin().await?;
    let n = crate::commands::ja4db::updatedb(&mut conn).await?;
    conn.commit().await?;
    let response = json!({
        "entries": n,
    });
    Ok(Json(response))
}

/// Add auto-archive filters, but to be extended.
///
/// For now just use the FilterEntry from configdb as the form
/// type. But that may need to change as we extend this.
pub(super) async fn add_filter(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(mut entry): Form<FilterEntry>,
) -> Result<impl IntoResponse, AppError> {
    let comment = entry.comment.take();
    let mut tx = context.configdb.pool.begin().await?;

    let key = format!(
        "{},{},{},{}",
        entry.sensor.as_ref().map_or("*", |v| v),
        &entry.src_ip.as_ref().map_or("*", |v| v),
        &entry.dest_ip.as_ref().map_or("*", |v| v),
        entry.signature_id
    );

    if let Ok(filters) = context.auto_archive.read() {
        if filters.has_key(&key) {
            info!("Arhive filters already contains key {}", &key);
            return Ok(Json(json!({})));
        }
    }

    let sql = "INSERT INTO filters (user_id, filter, comment) VALUES (?, ?, ?)";
    sqlx::query(sql)
        .bind(0)
        .bind(serde_json::to_value(&entry).unwrap())
        .bind(&comment)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let mut ingest = context.auto_archive.write().unwrap();
    ingest.add(&entry);

    info!(
        "New auto-archive filter added {:?} with comment: {:?}",
        &entry, &comment
    );

    Ok(Json(json!({})))
}

pub(super) async fn get_filters(
    _sesssion: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    let rows = context.configdb.get_filters().await?;
    Ok(Json(rows))
}

pub(super) async fn delete_filter(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(id): Path<u32>,
) -> Result<impl IntoResponse, AppError> {
    // Remove from database.
    let mut tx = context.configdb.pool.begin().await?;
    let row: Option<FilterRow> =
        sqlx::query_as::<_, FilterRow>("SELECT * FROM filters WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if row.is_some()
        && sqlx::query("DELETE FROM filters WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .is_ok()
    {
        tx.commit().await?;
    }

    // Remove from current ingest processing.
    if let Some(row) = row {
        let mut ingest = context.auto_archive.write().unwrap();
        ingest.remove(&row.filter.0);
    }

    Ok(Json(json!({})))
}
