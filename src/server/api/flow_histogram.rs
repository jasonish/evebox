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

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;

use crate::datastore::FlowHistogramParameters;
use crate::server::response::Response;
use crate::server::session::Session;
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

pub async fn handler(
    context: Arc<ServerContext>,
    query: Query,
    _session: Arc<Session>,
) -> Result<impl warp::Reply, warp::Rejection> {
    helpers::log_unknown_parameters("flow-histogram", &query.other);
    let params = FlowHistogramParameters {
        mints: helpers::mints_from_time_range(query.time_range, None)?,
        interval: query.interval,
        query_string: query.query_string,
    };
    context
        .datastore
        .flow_histogram(params)
        .await
        .map(|r| Ok(Response::Json(r)))?
}
