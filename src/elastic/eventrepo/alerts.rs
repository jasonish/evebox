// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use crate::server::autoarchive::AutoArchive;
use crate::sqlite::configdb::EventFilter;
use crate::{
    elastic::{AlertQueryOptions, ElasticResponse},
    eventrepo::{AggAlert, AggAlertMetadata, AlertsResult},
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
            if sensor == "(no-name)" {
                // Filter for documents without a host field
                filters.push(
                    json!({"bool": {"must_not": {"exists": {"field": self.map_field("host")}}}}),
                );
            } else {
                filters.push(json!({"term": {self.map_field("host"): sensor}}));
            }
        }

        if !has_min_timestamp && let Some(ts) = options.timestamp_gte {
            filters.push(json!({"range": {"@timestamp": {"gte": ts.to_elastic()}}}));
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
                // The following are included for "badges" of extra
                // info in the alert list.
                "dns.query",
                "dns.queries",
                "http.hostname",
                "quic.sni",
                "tls.sni",
                "host",
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
                            "terms": {
                                "field": self.map_field("src_ip"),
                                "size": 1000,
                                "missing": "null",
                            },
                            "aggs": {
                                "destinations": {
                                    "terms": {
                                        "field": self.map_field("dest_ip"),
                                        "size": 500,
                                        "missing": "null",
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

                                                    // ECS doesn't have timestamp.
                                                    "@timestamp",
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
        auto_archive: Arc<RwLock<AutoArchive>>,
    ) -> Result<AlertsResult> {
        let mut query = self.build_inbox_query(options);
        query["timeout"] = "3s".into();
        let start = std::time::Instant::now();
        let body = self.search(&query).await?.text().await?;
        let response: ElasticResponse = serde_json::from_str(&body)?;
        if let Some(error) = &response.error {
            bail!("elasticsearch: {}", error.first_reason());
        }

        debug!(
            "Elasticsearch alert query took {:?}, es-time: {}, response-size: {}, timed-out: {}",
            start.elapsed(),
            response.took,
            body.len(),
            response.timed_out,
        );

        // Lowest timestamp found.
        let mut from = None;

        // Largest timestamp found.
        let mut to = None;

        let mut alerts: Vec<AggAlert> = vec![];
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
                                    let oldest = bucket["oldest"]["hits"]["hits"][0].clone();

                                    if self.ecs {
                                        self.transform_ecs(&mut newest);
                                    }

                                    let escalated = &bucket["escalated"]["doc_count"]
                                        .as_u64()
                                        .ok_or_else(|| anyhow!("Missing escalated doc_count"))?;

                                    let min_timestamp = if oldest["_source"]["timestamp"]
                                        .is_string()
                                    {
                                        &oldest["_source"]["timestamp"]
                                    } else {
                                        &oldest["_source"]["@timestamp"]
                                    }
                                    .as_str()
                                    .ok_or_else(|| {
                                        anyhow!("No timestamp field on _source or not a string")
                                    })?;

                                    let min_timestamp =
                                        crate::datetime::parse(min_timestamp, None)?;

                                    let max_timestamp = if newest["_source"]["timestamp"]
                                        .is_string()
                                    {
                                        &newest["_source"]["timestamp"]
                                    } else {
                                        &newest["_source"]["@timestamp"]
                                    }
                                    .as_str()
                                    .ok_or_else(|| {
                                        anyhow!("No timestamp field on _source or not a string")
                                    })?;

                                    let max_timestamp =
                                        crate::datetime::parse(max_timestamp, None)?;

                                    let count = bucket["doc_count"]
                                        .as_u64()
                                        .ok_or_else(|| anyhow!("doc_count field missing"))?;

                                    let id = newest["_id"]
                                        .as_str()
                                        .ok_or_else(|| anyhow!("_id field missing"))?
                                        .to_string();
                                    let source = newest["_source"].take();

                                    // TODO: Do something with whats
                                    // left in newest. Perhaps an
                                    // "_elastic" field in the
                                    // response.

                                    if let Some(ts) = &from {
                                        if min_timestamp < *ts {
                                            from = Some(min_timestamp.clone());
                                        }
                                    } else {
                                        from = Some(min_timestamp.clone());
                                    }

                                    if let Some(ts) = &to {
                                        if max_timestamp > *ts {
                                            to = Some(max_timestamp.clone());
                                        }
                                    } else {
                                        to = Some(max_timestamp.clone());
                                    }

                                    let is_archived = source["tags"]
                                        .as_array()
                                        .map(|a| a.iter().any(|t| t == "evebox.archived"))
                                        .unwrap_or(false);

                                    if !is_archived {
                                        let matched_filter = auto_archive
                                            .read()
                                            .unwrap()
                                            .matching_filter(&source)
                                            .cloned();
                                        if let Some(filter) = matched_filter {
                                            if let Some(tx) = &self.auto_archive_tx {
                                                let _ = tx.send(filter);
                                            }
                                            continue;
                                        }
                                    }

                                    let alert = AggAlert {
                                        id,
                                        source,
                                        metadata: AggAlertMetadata {
                                            count,
                                            escalated_count: *escalated,
                                            min_timestamp,
                                            max_timestamp,
                                        },
                                    };
                                    alerts.push(alert);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            warn!("Elasticsearch response has no aggregations");
        }

        let response = AlertsResult {
            ecs: self.ecs,
            events: alerts,
            took: response.took,
            timed_out: response.timed_out,
            min_timestamp: from,
            max_timestamp: to,
        };

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use crate::elastic::{AlertQueryOptions, Client, ElasticEventRepo};
    use serde_json::json;

    fn test_repo() -> ElasticEventRepo {
        ElasticEventRepo::new(
            "test".to_string(),
            "test-*".to_string(),
            Client::new("http://localhost:9200"),
            false,
            false,
        )
    }

    fn array_contains(value: &serde_json::Value, needle: &serde_json::Value) -> bool {
        value
            .as_array()
            .map(|a| a.contains(needle))
            .unwrap_or(false)
    }

    fn inbox_query(query_string: &str) -> serde_json::Value {
        test_repo().build_inbox_query(AlertQueryOptions {
            query_string: Some(query_string.to_string()),
            ..Default::default()
        })
    }

    #[test]
    fn is_archived_filters_on_tag() {
        let query = inbox_query("is:archived");
        assert!(array_contains(
            &query["query"]["bool"]["filter"],
            &json!({"term": {"tags": "evebox.archived"}})
        ));
    }

    #[test]
    fn not_archived_uses_must_not() {
        let query = inbox_query("-is:archived");
        assert!(array_contains(
            &query["query"]["bool"]["must_not"],
            &json!({"term": {"tags": "evebox.archived"}})
        ));
    }

    #[test]
    fn is_escalated_filters_on_tag() {
        let query = inbox_query("is:escalated");
        assert!(array_contains(
            &query["query"]["bool"]["filter"],
            &json!({"term": {"tags": "evebox.escalated"}})
        ));
    }

    #[test]
    fn not_escalated_uses_must_not() {
        let query = inbox_query("-is:escalated");
        assert!(array_contains(
            &query["query"]["bool"]["must_not"],
            &json!({"term": {"tags": "evebox.escalated"}})
        ));
    }
}
