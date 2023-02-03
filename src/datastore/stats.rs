// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::Datastore;
use super::StatsAggQueryParams;
use anyhow::Result;

impl Datastore {
    pub async fn stats_agg(&self, params: &StatsAggQueryParams) -> Result<serde_json::Value> {
        match self {
            Datastore::Elastic(ds) => ds.stats_agg(params).await,
            Datastore::SQLite(ds) => ds.stats_agg(params).await,
        }
    }

    pub async fn stats_agg_diff(&self, params: &StatsAggQueryParams) -> Result<serde_json::Value> {
        match self {
            Datastore::Elastic(ds) => ds.stats_agg_diff(params).await,
            Datastore::SQLite(ds) => ds.stats_agg_diff(params).await,
        }
    }
}
