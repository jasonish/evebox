// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::{
    request::{term_filter, timestamp_lte_filter},
    ElasticEventRepo,
};
use crate::elastic::DateTime;
use crate::error::AppError;

impl ElasticEventRepo {
    pub(crate) async fn dns_reverse_lookup(
        &self,
        before: Option<DateTime>,
        sensor: Option<String>,
        src_ip: String,
        dest_ip: String,
    ) -> Result<serde_json::Value, AppError> {
        let mut filters = vec![];

        filters.push(term_filter(&self.map_field("event_type"), "dns"));

        if let Some(before) = before {
            filters.push(timestamp_lte_filter(&before));
        }

        if let Some(host) = sensor {
            filters.push(term_filter(&self.map_field("host"), &host));
        }

        filters.push(json!({
            "bool": {
                "should": [
                    term_filter(&self.map_field("src_ip"), &src_ip),
                    term_filter(&self.map_field("src_ip"), &dest_ip),
                    term_filter(&self.map_field("dest_ip"), &src_ip),
                    term_filter(&self.map_field("dest_ip"), &dest_ip),
                ],
            }
        }));

        filters.push(json!({
            "bool": {
                "should": [
                    term_filter(&self.map_field("dns.type"), "response"),
                    term_filter(&self.map_field("dns.type"), "answer")
                ],
            }
        }));

        filters.push(term_filter(&self.map_field("dns.answers.rdata"), &src_ip));

        let aggs = json!({
            "answers": {
                "terms": {"field": &self.map_field("dns.queries.rrname.keyword")}
            },
            "old_answers": {
                "terms": {"field": &self.map_field("dns.rrname.keyword")}
            },
        });

        let request = json!({
            "query": {
                "bool": {
                    "filter": filters,
                }
            },
            "aggs": aggs,
            "size": 0,
        });

        let mut rrnames: Vec<String> = vec![];

        let response: serde_json::Value = self.search(&request).await?.json().await?;
        let answers = &response["aggregations"]["answers"]["buckets"];
        if let Some(buckets) = answers.as_array() {
            for bucket in buckets {
                if let Some(key) = bucket["key"].as_str() {
                    rrnames.push(key.to_string());
                }
            }
        }

        if let Some(buckets) = response["aggregations"]["old_answers"]["buckets"].as_array() {
            for bucket in buckets {
                if let Some(key) = bucket["key"].as_str() {
                    rrnames.push(key.to_string());
                }
            }
        }

        Ok(json!({
            "rrnames": rrnames,
        }))
    }
}
