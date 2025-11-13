// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use axum::extract::Path;

use super::prelude::*;
use crate::prelude::*;

pub(crate) async fn indices(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    match &context.datastore {
        crate::eventrepo::EventRepo::SQLite(_) => Err(AppError::InternalServerError),
        crate::eventrepo::EventRepo::Elastic(elastic) => {
            let stats = elastic
                .get_client()
                .get_index_stats(elastic.get_base_index())
                .await?;
            Ok(axum::Json(stats))
        }
    }
}

pub(crate) async fn delete(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    match &context.datastore {
        crate::eventrepo::EventRepo::SQLite(_) => Err(AppError::InternalServerError),
        crate::eventrepo::EventRepo::Elastic(elastic) => {
            info!("Deleting index: {}", name);
            let status = elastic.get_client().delete_index(&name).await?;
            let status = status.as_u16();
            let status = StatusCode::from_u16(status).map_err(|_| {
                AppError::StringError("invalid status code returned from elasticsearch".to_string())
            })?;
            Ok(status.into_response())
        }
    }
}
