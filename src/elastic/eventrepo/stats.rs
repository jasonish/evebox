// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::ElasticEventRepo;
use crate::{datetime::DateTime, eventrepo::StatsAggQueryParams, util};
use anyhow::Result;
use serde::{Deserialize, Serialize};

impl ElasticEventRepo {
    pub async fn stats_agg(&self, params: &StatsAggQueryParams) -> Result<serde_json::Value> {
        let range = DateTime::now().datetime - params.start_time.datetime;
        let range = range.num_seconds();
        let interval = util::histogram_interval(range);

        let start_time = params.start_time.to_rfc3339_utc();
        let mut filters = vec![];
        filters.push(json!({"term": {self.map_field("event_type"): "stats"}}));
        filters.push(json!({"range": {"@timestamp": {"gte": start_time}}}));
        if let Some(sensor_name) = &params.sensor_name {
            filters.push(json!({"term": {"host": sensor_name}}));
        }
        let field = self.map_field(&params.field);
        let query = json!({
           "query": {
                "bool": {
                    "filter": filters,
                }
            },
            "size": 0,
            "sort": [{"@timestamp": {"order": "asc"}}],
            "aggs": {
               "histogram": {
                  "date_histogram": {
                    "field": "@timestamp",
                      "fixed_interval": format!("{interval}s"),
                    },
              "aggs": {
                "memuse": {
                  "max": {
                    "field": field,
                  }
                },
            }}}
        });
        let mut response: serde_json::Value = self.search(&query).await?.json().await?;

        #[derive(Debug, Deserialize, Serialize, Default)]
        struct Value {
            value: Option<f64>,
        }

        #[derive(Debug, Deserialize, Serialize, Default)]
        struct Bucket {
            key_as_string: String,
            memuse: Value,
        }

        let buckets = response["aggregations"]["histogram"]["buckets"].take();
        let buckets: Vec<Bucket> = serde_json::from_value(buckets)?;
        let buckets: Vec<serde_json::Value> = buckets
            .iter()
            .map(|b| {
                json!({
                    "timestamp": b.key_as_string,
                    "value": b.memuse.value.unwrap_or(0.0) as u64,
                })
            })
            .collect();
        let response = json!({
            "data": buckets,
        });

        Ok(response)
    }

    pub async fn stats_agg_diff(&self, params: &StatsAggQueryParams) -> Result<serde_json::Value> {
        let start_time = params.start_time.to_rfc3339_utc();
        let range = (DateTime::now().datetime - params.start_time.datetime).num_seconds();
        let interval = crate::util::histogram_interval(range);
        let mut filters = vec![];
        filters.push(json!({"term": {self.map_field("event_type"): "stats"}}));
        filters.push(json!({"range": {"@timestamp": {"gte": start_time}}}));
        if let Some(sensor_name) = &params.sensor_name {
            filters.push(json!({"term": {self.map_field("host"): sensor_name}}));
        }
        let field = self.map_field(&params.field);
        let query = json!({
          "query": {
            "bool": {
              "filter": filters,
            }
          },
          "size": 0,
          "sort": [{"@timestamp": {"order": "asc"}}],
          "aggs": {
            "histogram": {
              "date_histogram": {
                "field": "@timestamp",
                  "fixed_interval": format!("{interval}s"),
              },
              "aggs": {
                "values": {
                  "max": {
                    "field": field,
                  }
                },
                "values_deriv": {
                  "derivative": {
                    "buckets_path": "values"
                  }
                }
              }
            }
          }
        });

        let response = self.search(&query).await?;
        if response.status() != 200 {
            let error_text = response.text().await?;
            anyhow::bail!(error_text);
        }
        let mut response: serde_json::Value = response.json().await?;

        #[derive(Debug, Deserialize, Serialize, Default)]
        struct Value {
            value: Option<f64>,
        }

        #[derive(Debug, Deserialize, Serialize)]
        struct Bucket {
            key_as_string: String,
            key: u64,
            values_deriv: Option<Value>,
        }

        let buckets = response["aggregations"]["histogram"]["buckets"].take();
        let buckets: Vec<Bucket> = serde_json::from_value(buckets)?;
        let response_data: Vec<serde_json::Value> = buckets
            .iter()
            .map(|b| {
                let bytes = b
                    .values_deriv
                    .as_ref()
                    .and_then(|v| v.value.as_ref())
                    .copied()
                    .unwrap_or(0.0) as u64;
                json!({
                    "timestamp": b.key_as_string,
                    "value": bytes,
                })
            })
            .collect();
        let response = json!({
            "data": response_data,
        });
        Ok(response)
    }
}
