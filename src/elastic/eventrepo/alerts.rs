// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use axum::{response::IntoResponse, Json};
use tracing::{error, info, warn};

use crate::{
    elastic::{AlertQueryOptions, ElasticResponse},
    eventrepo::DatastoreError,
    queryparser::{self, QueryValue},
};

use super::{ElasticEventRepo, MINIMUM_SHOULD_MATCH};

impl ElasticEventRepo {
    pub fn build_inbox_query(&self, options: AlertQueryOptions) -> serde_json::Value {
        let mut filters = Vec::new();
        let mut should = Vec::new();
        let mut must_not = Vec::new();

        // Set to true if the min timestamp is set in the query string
        let mut has_min_timestamp = false;

        if let Some(q) = &options.query_string {
            // TODO: Need client tz_offset here.
            match queryparser::parse(q, None) {
                Ok(elements) => {
                    self.apply_query_string(&elements, &mut filters, &mut should, &mut must_not);
                    has_min_timestamp = elements
                        .iter()
                        .any(|e| matches!(&e.value, QueryValue::From(_)));
                }
                Err(err) => {
                    error!(
                        "Failed to parse query string: error={}, query-string={}",
                        q, err
                    );
                }
            }
        }

        filters.push(json!({"exists": {"field": self.map_field("event_type")}}));
        filters.push(json!({"term": {self.map_field("event_type"): "alert"}}));

        if let Some(sensor) = &options.sensor {
            filters.push(json!({"term": {self.map_field("host"): sensor}}));
        }

        if !has_min_timestamp {
            if let Some(ts) = options.timestamp_gte {
                filters.push(json!({"range": {"@timestamp": {"gte": ts.to_elastic()}}}));
            }
        }

        for tag in options.tags {
            if let Some(tag) = tag.strip_prefix('-') {
                let j = json!({"term": {"tags": tag}});
                must_not.push(j);
            } else {
                let j = json!({"term": {"tags": tag}});
                filters.push(j);
            }
        }

        // Reducing the source size from ECS is not done yet.
        let source = if self.ecs {
            json!([])
        } else {
            json!([
                "alert.action",
                "alert.severity",
                "alert.signature",
                "alert.signature_id",
                "app_proto",
                "dest_ip",
                "src_ip",
                "tags",
                "timestamp",
            ])
        };

        let mut query = json!({
            "query": {
                "bool": {
                    "filter": filters,
                    "must_not": must_not,
                }
            },
            "sort": [{"@timestamp": {"order": "desc"}}],
            "aggs": {
                "signatures": {
                    "terms": {"field": self.map_field("alert.signature_id"), "size": 2000},
                    "aggs": {
                        "sources": {
                            "terms": {"field": self.map_field("src_ip"), "size": 1000},
                            "aggs": {
                                "destinations": {
                                    "terms": {
                                        "field": self.map_field("dest_ip"),
                                        "size": 500
                                    },
                                    "aggs": {
                                        "escalated": {
                                            "filter": {
                                                "term": {
                                                    "tags": "evebox.escalated",
                                                }
                                            }
                                        },
                                        "newest": {
                                            "top_hits": {
                                                "size": 1,
                                                "sort": [
                                                    {
                                                        self.map_field("timestamp"): {"order": "desc"}
                                                    }
                                                ],
                                                "_source": source,
                                            }
                                        },
                                        "oldest": {
                                            "top_hits": {
                                                "size": 1,
                                                "sort": [
                                                    {
                                                        self.map_field("timestamp"): {"order": "asc"}
                                                    }
                                                ],
                                                // We only need the
                                                // timestamp from the
                                                // oldest event.
                                                "_source": [
                                                    "timestamp",
                                                ]
                                            }
                                        }
                                    },
                                },
                            },
                        },
                    },
                }
            }
        });

        if !should.is_empty() {
            query["query"]["bool"]["should"] = should.into();
            query["query"]["bool"][MINIMUM_SHOULD_MATCH] = 1.into();
        }

        query
    }

    pub async fn alerts(
        &self,
        options: AlertQueryOptions,
    ) -> Result<impl IntoResponse, DatastoreError> {
        let query = self.build_inbox_query(options);
        let start = std::time::Instant::now();
        let body = self.search(&query).await?.text().await?;
        let response: ElasticResponse = serde_json::from_str(&body)?;
        if let Some(error) = response.error {
            return Err(DatastoreError::ElasticSearchError(error.first_reason()));
        }

        info!(
            "Elasticsearch alert query took {:?}, es-time: {}, response-size: {}",
            start.elapsed(),
            response.took,
            body.len()
        );

        let mut alerts = Vec::new();
        if let Some(aggregrations) = response.aggregations {
            if let serde_json::Value::Array(buckets) = &aggregrations["signatures"]["buckets"] {
                for bucket in buckets {
                    if let serde_json::Value::Array(buckets) = &bucket["sources"]["buckets"] {
                        for bucket in buckets {
                            if let serde_json::Value::Array(buckets) =
                                &bucket["destinations"]["buckets"]
                            {
                                for bucket in buckets {
                                    let mut newest = bucket["newest"]["hits"]["hits"][0].clone();
                                    let mut oldest = bucket["oldest"]["hits"]["hits"][0].clone();

                                    if self.ecs {
                                        self.transform_ecs(&mut newest);
                                        self.transform_ecs(&mut oldest);
                                    }

                                    let escalated = &bucket["escalated"]["doc_count"];

                                    newest["_metadata"] = json!({
                                        "count": bucket["doc_count"],
                                        "escalated_count": escalated,
                                        "min_timestamp": &oldest["_source"]["timestamp"],
                                        "max_timestamp": &newest["_source"]["timestamp"],
                                        "aggregate": true,
                                    });
                                    alerts.push(newest);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            warn!("Elasticsearch response has no aggregations");
        }

        let response = json!({
            "ecs": self.ecs,
            "events": alerts,
        });

        Ok(Json(response))
    }
}
