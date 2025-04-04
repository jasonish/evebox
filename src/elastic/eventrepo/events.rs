// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use super::ElasticEventRepo;
use crate::elastic::request;
use crate::eventrepo::{self};
use crate::LOG_QUERIES;
use serde_json::json;

const MINIMUM_SHOULD_MATCH: &str = "minimum_should_match";

impl ElasticEventRepo {
    pub async fn events(&self, params: eventrepo::EventQueryParams) -> Result<serde_json::Value> {
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

        if let Some(ts) = params.from {
            filters.push(request::timestamp_gte_filter(&ts));
        }

        if let Some(ts) = params.to {
            filters.push(request::timestamp_lte_filter(&ts));
        }

        let sort_by = params.sort_by.unwrap_or_else(|| "@timestamp".to_string());
        let sort_order = params.order.unwrap_or_else(|| "desc".to_string());
        let size = params.size.unwrap_or(500);

        let mut body = json!({
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

                if let Some(ja4) = hit["_source"]["tls"]["ja4"].as_str() {
                    if let Some(configdb) = crate::server::context::get_configdb() {
                        let sql = "SELECT data FROM ja4db WHERE fingerprint = ?";
                        let info: Result<Option<serde_json::Value>, _> = sqlx::query_scalar(sql)
                            .bind(ja4)
                            .fetch_optional(&configdb.pool)
                            .await;
                        if let Ok(Some(info)) = info {
                            hit["_source"]["ja4db"] = info;
                        }
                    }
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
