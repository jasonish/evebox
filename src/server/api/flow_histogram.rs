// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::prelude::*;

use axum::extract::{Extension, Form};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;

use crate::datastore::FlowHistogramParameters;
use crate::server::main::SessionExtractor;
use crate::server::ServerContext;

use super::helpers;

#[derive(Debug, Deserialize)]
pub struct Query {
    pub time_range: Option<String>,
    pub interval: Option<String>,
    pub query_string: Option<String>,
    #[serde(flatten)]
    pub other: HashMap<String, String>,
}

pub(crate) async fn handler(
    Extension(context): Extension<Arc<ServerContext>>,
    SessionExtractor(_session): SessionExtractor,
    Form(query): Form<Query>,
) -> impl IntoResponse {
    helpers::log_unknown_parameters("flow-histogram", &query.other);
    let params = FlowHistogramParameters {
        mints: helpers::mints_from_time_range(query.time_range, None).unwrap(),
        interval: query.interval,
        query_string: query.query_string,
    };
    match context.datastore.flow_histogram(params).await {
        Err(err) => {
            error!("Flow histogram failed: {:?}", err);
            return (StatusCode::INTERNAL_SERVER_ERROR, "").into_response();
        }
        Ok(response) => Json(response).into_response(),
    }
}
