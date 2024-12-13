// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::ElasticEventRepo;
use crate::elastic::request;
use crate::eventrepo::{self, DatastoreError};
use crate::LOG_QUERIES;
use serde_json::json;
use tracing::info;
use tracing::warn;

const MINIMUM_SHOULD_MATCH: &str = "minimum_should_match";

impl ElasticEventRepo {
    pub async fn events(
        &self,
        params: eventrepo::EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        let mut filters = vec![request::exists_filter(&self.map_field("event_type"))];
        let mut should = vec![];
        let mut must_not = vec![];

        if let Some(event_type) = params.event_type {
            filters.push(request::term_filter(
                &self.map_field("event_type"),
                &event_type,
            ));
        }

        self.apply_query_string(
            &params.query_string,
            &mut filters,
            &mut should,
            &mut must_not,
        );

        if let Some(ts) = params.min_timestamp {
            warn!("Unexpected min_timestamp of {}", &ts);
        }

        if let Some(ts) = params.max_timestamp {
            warn!("Unexpected max_timestamp of {}", &ts);
        }

        let sort_by = params.sort_by.unwrap_or_else(|| "@timestamp".to_string());
        let sort_order = params.order.unwrap_or_else(|| "desc".to_string());
        let size = params.size.unwrap_or(500);

        let mut body = json!({
            "runtime_mappings": self.runtime_mappings(),
            "query": {
                "bool": {
                    "filter": filters,
                    "must_not": must_not,
                }
            },
            "sort": [{sort_by: {"order": sort_order}}],
            "size": size,
        });

        if !should.is_empty() {
            body["query"]["bool"]["should"] = should.into();
            body["query"]["bool"][MINIMUM_SHOULD_MATCH] = 1.into();
        }

        if *LOG_QUERIES {
            info!("{}", &body);
        }

        let response = self.search(&body).await?;
        let response: serde_json::Value = response.json().await?;

        if response["error"].is_object() {
            // Attempt to convert the response back to JSON. We could
            // just use the body, but its full of new line feeds, etc.
            let error = match serde_json::to_string(&response) {
                Ok(error) => error,
                Err(_) => format!("{:?}", &response),
            };

            return Err(anyhow::anyhow!(
                "Elasticsearch returned error on event query: error={}",
                error
            ))?;
        }

        // Another way we can get errors from
        // Elasticsearch/Opensearch, even with a 200 status code.
        if let Some(failure) = response["_shards"]["failures"]
            .as_array()
            .and_then(|v| v.first())
        {
            warn!(
                "Elasticsearch reported failures, the first being: {:?}",
                failure
            );
        }

        let hits = &response["hits"]["hits"];

        let mut events = vec![];
        if let Some(hits) = hits.as_array() {
            for hit in hits {
                let mut hit = hit.clone();
                if self.ecs {
                    self.transform_ecs(&mut hit);
                }
                events.push(hit);
            }
        }

        let response = json!({
            "ecs": self.ecs,
            "events": events,
        });

        Ok(response)
    }
}
