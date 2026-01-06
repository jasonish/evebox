// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{elastic::ElasticEventRepo, prelude::*};

use std::sync::Arc;

use axum::Json;
use axum::{Extension, response::IntoResponse};
use axum_extra::extract::Form;
use serde::Serialize;

use crate::{
    queryparser,
    sqlite::{builder::EventQueryBuilder, eventrepo::SqliteEventRepo},
};

use super::{AppError, QueryElement, ServerContext, SessionExtractor};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct OccurrencesOfForm {
    pub q: Option<String>,
}

pub(crate) async fn count(
    _session: SessionExtractor,
    Extension(context): Extension<Arc<ServerContext>>,
    Form(form): Form<OccurrencesOfForm>,
) -> Result<impl IntoResponse, AppError> {
    let q = form
        .q
        .clone()
        .map(|q| queryparser::parse(&q, None))
        .transpose()?
        .unwrap_or_default();
    if q.is_empty() {
        return Ok((StatusCode::BAD_REQUEST, "query required").into_response());
    }

    let result = match &context.datastore {
        crate::eventrepo::EventRepo::Elastic(ds) => elastic_count(ds, q).await?,
        crate::eventrepo::EventRepo::SQLite(ds) => sqlite_count(ds, q).await?,
        crate::eventrepo::EventRepo::Postgres(_) => todo!("count for postgres"),
    };

    Ok(Json(result).into_response())
}

#[derive(Debug, Default, Serialize)]
struct CountResult {
    total: i64,
}

async fn elastic_count(ds: &ElasticEventRepo, q: Vec<QueryElement>) -> anyhow::Result<CountResult> {
    let mut filter = vec![];
    let mut should = vec![];
    let mut must_not = vec![];

    filter.push(crate::elastic::request::exists_filter(
        &ds.map_field("event_type"),
    ));
    ds.apply_query_string(&q, &mut filter, &mut should, &mut must_not);

    let query = json!({
        "query": {
            "bool": {
                "filter": filter,
                "must_not": must_not,
            }
        },
        "size": 0,
        "track_total_hits": true,
    });

    let response: serde_json::Value = ds.search(&query).await?.json().await?;

    let total = response["hits"]["total"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("Elasticsearch response had no field hits.total"))?;
    let result = CountResult { total };

    Ok(result)
}

async fn sqlite_count(ds: &SqliteEventRepo, q: Vec<QueryElement>) -> anyhow::Result<CountResult> {
    let mut builder = EventQueryBuilder::new(ds.fts().await);
    builder.select("count(*)");
    builder.from("events");
    builder.apply_query_string(&q)?;
    let (sql, args) = builder.build()?;
    let total: i64 = sqlx::query_scalar_with(&sql, args)
        .fetch_one(ds.get_pool())
        .await?;
    let result = CountResult { total };
    Ok(result)
}
