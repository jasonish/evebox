// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

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
    dbg!(&query);
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
