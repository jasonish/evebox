// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use std::sync::Arc;

use axum::Json;
use axum::{Extension, response::IntoResponse};
use axum_extra::extract::Form;
use serde::Serialize;

use crate::queryparser;

use super::{AppError, ServerContext, SessionExtractor};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct OccurrencesOfForm {
    pub q: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct CountResult {
    total: u64,
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

    let total = context.datastore.count(&q).await?;
    Ok(Json(CountResult { total }).into_response())
}
