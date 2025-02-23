// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use axum::extract::Path;

use crate::elastic::ElasticEventRepo;

use super::prelude::*;
use crate::prelude::*;

pub(crate) async fn indices(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
) -> Result<impl IntoResponse, AppError> {
    match &context.datastore {
        crate::eventrepo::EventRepo::SQLite(_) => Err(AppError::InternalServerError),
        crate::eventrepo::EventRepo::Elastic(elastic) => {
            let names = get_index_stats(elastic).await?;
            Ok(axum::Json(names))
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
            let response = elastic.client.delete(&name)?.send().await?;
            let status = response.status().as_u16();
            let status = StatusCode::from_u16(status).map_err(|_| {
                AppError::StringError("invalid status code returned from elasticsearch".to_string())
            })?;
            Ok(status.into_response())
        }
    }
}

async fn get_index_stats(elastic: &ElasticEventRepo) -> Result<Vec<SimpleIndexStats>> {
    let path = format!("{}*/_stats", &elastic.base_index);
    let response = elastic.client.get(&path)?.send().await?;
    let json: serde_json::Value = response.json().await?;
    let indices: HashMap<String, IndexStatsResponse> =
        serde_json::from_value(json["indices"].clone())?;
    let mut keys: Vec<&String> = indices.keys().collect();
    keys.sort();
    let mut simple = vec![];
    for key in keys {
        simple.push(SimpleIndexStats {
            name: key.clone(),
            doc_count: indices[key].primaries.docs.count,
            store_size: indices[key].primaries.store.size_in_bytes,
        });
    }

    Ok(simple)
}

#[derive(Debug, Clone, Deserialize)]
struct IndexStatsResponse {
    primaries: IndexStatsPrimaries,
}

#[derive(Debug, Clone, Deserialize)]
struct IndexStatsPrimaries {
    docs: IndexStatsDocs,
    store: IndexStatsStore,
}

#[derive(Debug, Clone, Deserialize)]
struct IndexStatsDocs {
    count: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct IndexStatsStore {
    size_in_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
struct SimpleIndexStats {
    name: String,
    doc_count: u64,
    store_size: u64,
}
