// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Polling Elasticsearch and OpenSearch for the live feed, for events
//! written to the datastore by something other than EveBox.

use super::ElasticEventRepo;
use crate::elastic::request;
use crate::prelude::*;
use crate::server::livefeed::{DocRef, FeedSource, Millis, ScanPage, ScannedDoc};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

/// Longest a live feed request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Longest response body excerpt kept in an error.
const MAX_BODY_EXCERPT: usize = 200;

/// Every live feed search asks for the same copies of each shard, so a
/// fetch sees what the scan before it saw, and documents with equal
/// timestamps come back in the same order from one scan to the next.
const PREFERENCE: &str = "evebox-live-feed";

impl ElasticEventRepo {
    /// Search the event indices for the live feed.
    ///
    /// Fails if no shard that ran the search succeeded. Shards that failed
    /// otherwise are reported as the returned reason, as the answer is
    /// then from part of the data. Errors are shown to users, so they
    /// never include the URL, which can carry credentials.
    async fn live_search(
        &self,
        body: &serde_json::Value,
    ) -> Result<(serde_json::Value, Option<String>)> {
        let path = format!("{}/_search?preference={PREFERENCE}", self.index_pattern);
        let response = self
            .client
            .post(&path)
            .map_err(request_error)?
            .json(body)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        let text = response.text().await.map_err(request_error)?;
        let response: serde_json::Value = match serde_json::from_str(&text) {
            Ok(response) => response,
            Err(_) => bail!("HTTP {status}: {}", excerpt(&text)),
        };
        if response["error"].is_object() || response["error"].is_string() {
            bail!("{}", error_reason(&response["error"]));
        }
        if !status.is_success() {
            bail!("HTTP {status}: {}", excerpt(&text));
        }
        let shards = &response["_shards"];
        let failed = shards["failed"].as_u64().unwrap_or(0);
        let mut partial = None;
        if failed > 0 {
            let failure = &shards["failures"][0];
            let reason = format!(
                "search failed on {failed} shards, the first in {}: {}",
                failure["index"].as_str().unwrap_or("an unknown index"),
                error_reason(&failure["reason"]),
            );
            // Shards skipped as having nothing in range count as
            // successful, so "all failed" can come with failed < total.
            let successful = shards["successful"].as_u64().unwrap_or(0);
            let skipped = shards["skipped"].as_u64().unwrap_or(0);
            if successful <= skipped {
                bail!("{reason}");
            }
            partial = Some(reason);
        } else if response["timed_out"].as_bool() == Some(true) {
            partial = Some("search timed out".to_string());
        }
        Ok((response, partial))
    }
}

#[async_trait::async_trait]
impl FeedSource for ElasticEventRepo {
    async fn scan_page(&self, gte: Millis, lt: Millis, size: usize) -> Result<ScanPage> {
        // Identities only. Sorting on the event timestamp gives it back as
        // the sort value, in milliseconds even for date_nanos fields
        // (`numeric_type`; without it, nanoseconds would sort ahead of
        // every millisecond date), and `unmapped_type` keeps an index
        // without the field from failing. The range is half open in whole
        // milliseconds, matching the sort values, which are truncated.
        //
        // The event type filter uses the field as mapped, not its keyword
        // sub-field, as which of those to use can change once the field
        // types load, and the filter must not change under a running feed.
        let body = json!({
            "size": size,
            "_source": false,
            "track_total_hits": false,
            "query": {
                "bool": {
                    "filter": [
                        request::exists_filter(&self.base_field("event_type")),
                        {"range": {"@timestamp": {"gte": gte, "lt": lt, "format": "epoch_millis"}}},
                    ]
                }
            },
            "sort": [
                {"@timestamp": {"order": "desc", "unmapped_type": "date", "numeric_type": "date"}}
            ],
        });
        let (response, partial) = self.live_search(&body).await?;
        let Some(hits) = response["hits"]["hits"].as_array() else {
            bail!("unexpected search response, no hits");
        };
        let mut docs = Vec::with_capacity(hits.len());
        for hit in hits {
            let (Some(index), Some(id), Some(ts)) = (
                hit["_index"].as_str(),
                hit["_id"].as_str(),
                sort_millis(&hit["sort"][0]),
            ) else {
                bail!("unexpected search response, a hit without an index, id or timestamp");
            };
            docs.push(ScannedDoc {
                doc: DocRef {
                    index: index.to_string(),
                    id: id.to_string(),
                },
                ts,
            });
        }
        Ok(ScanPage { docs, partial })
    }

    async fn fetch(&self, docs: &[DocRef]) -> Result<Vec<(DocRef, serde_json::Value)>> {
        if docs.is_empty() {
            return Ok(vec![]);
        }
        // Matched by index and id together: the same id can be in more
        // than one index. A search rather than a multi-get, so documents
        // routed to a shard by something other than their id are found.
        let mut by_index: HashMap<&str, Vec<&str>> = HashMap::new();
        for doc in docs {
            by_index.entry(&doc.index).or_default().push(&doc.id);
        }
        let should: Vec<serde_json::Value> = by_index
            .into_iter()
            .map(|(index, ids)| {
                json!({"bool": {"filter": [
                    {"term": {"_index": index}},
                    {"ids": {"values": ids}},
                ]}})
            })
            .collect();
        let body = json!({
            "size": docs.len(),
            "track_total_hits": false,
            "query": {"bool": {"should": should, "minimum_should_match": 1}},
        });
        let (mut response, partial) = self.live_search(&body).await?;
        if let Some(partial) = partial {
            // What's missing is skipped, rather than retried forever.
            debug!("Live feed: fetch answered from part of the data: {partial}");
        }
        let Some(hits) = response["hits"]["hits"].as_array_mut() else {
            bail!("unexpected search response, no hits");
        };
        let mut events = Vec::with_capacity(hits.len());
        for hit in hits.iter_mut() {
            let (Some(index), Some(id)) = (hit["_index"].as_str(), hit["_id"].as_str()) else {
                continue;
            };
            let doc = DocRef {
                index: index.to_string(),
                id: id.to_string(),
            };
            if self.ecs {
                self.transform_ecs(hit);
            }
            let mut event = hit["_source"].take();
            if !event.is_object() {
                continue;
            }
            normalize(&mut event, self.ecs);
            events.push((doc, event));
        }
        Ok(events)
    }
}

/// Shape a document like an event as EveBox ingests it, which is what the
/// live views and the firehose filters expect.
fn normalize(event: &mut serde_json::Value, ecs: bool) {
    if ecs {
        // EveBox looks for EVE fields under suricata.eve in ECS documents;
        // fill in what the conversion to ECS left there.
        if let Some(eve) = event["suricata"]["eve"].as_object().cloned() {
            for (key, value) in eve {
                fill_missing(&mut event[&key], value);
            }
        }
        // The sensor is the agent name with ECS, as in the rest of EveBox.
        if let Some(name) = event["agent"]["name"].as_str().map(str::to_string) {
            event["host"] = name.into();
        }
    }
    if !event["timestamp"].is_string() && event["@timestamp"].is_string() {
        event["timestamp"] = event["@timestamp"].clone();
    }
    // The sensor name: some shippers have `host` as an object.
    if !event["host"].is_string()
        && let Some(sensor) = crate::server::routing::sensor_identity(event).map(str::to_string)
    {
        event["host"] = sensor.into();
    }
}

/// Set what `target` is missing from `value`, keeping what it has.
fn fill_missing(target: &mut serde_json::Value, value: serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) if target.is_object() => {
            for (key, value) in fields {
                fill_missing(&mut target[&key], value);
            }
        }
        value if target.is_null() => *target = value,
        _ => {}
    }
}

/// A timestamp sort value in milliseconds. A long, but accept a float or
/// a string of digits too.
fn sort_millis(value: &serde_json::Value) -> Option<Millis> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|f| f as Millis))
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

/// The most specific reason in an Elasticsearch error.
fn error_reason(error: &serde_json::Value) -> String {
    if let Some(error) = error.as_str() {
        return error.to_string();
    }
    let cause = &error["root_cause"][0];
    let reason = cause["reason"]
        .as_str()
        .or_else(|| error["reason"].as_str())
        .unwrap_or("unknown error");
    match cause["type"].as_str().or_else(|| error["type"].as_str()) {
        Some(kind) => format!("{kind}: {reason}"),
        None => reason.to_string(),
    }
}

fn request_error(err: reqwest::Error) -> anyhow::Error {
    let timeout = err.is_timeout();
    let err = anyhow::Error::new(err.without_url());
    if timeout {
        err.context("request timed out")
    } else {
        err.context("request failed")
    }
}

fn excerpt(text: &str) -> String {
    let text = text.trim();
    let mut end = text.len().min(MAX_BODY_EXCERPT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn normalize_fills_in_timestamp_and_sensor() {
        let mut ecs = json!({
            "@timestamp": "2026-11-08T01:02:03.456Z",
            "host": {"name": "os-hostname"},
            "agent": {"name": "sensor-a"},
        });
        normalize(&mut ecs, true);
        assert_eq!(ecs["timestamp"], "2026-11-08T01:02:03.456Z");
        assert_eq!(ecs["host"], "sensor-a");

        let mut eve = json!({
            "timestamp": "2026-11-08T01:02:03.456789+0000",
            "@timestamp": "2026-11-08T01:02:03.456Z",
            "host": "sensor-b",
        });
        normalize(&mut eve, false);
        assert_eq!(eve["timestamp"], "2026-11-08T01:02:03.456789+0000");
        assert_eq!(eve["host"], "sensor-b");

        // A shipper's host object, without ECS.
        let mut shipped = json!({"host": {"name": "shipper-host"}});
        normalize(&mut shipped, false);
        assert_eq!(shipped["host"], "shipper-host");

        let mut bare = json!({"event_type": "alert"});
        normalize(&mut bare, false);
        assert_eq!(bare, json!({"event_type": "alert"}));
    }

    #[test]
    fn normalize_fills_in_eve_fields_from_suricata_eve_with_ecs() {
        let mut event = json!({
            "agent": {"name": "sensor-a"},
            // A string host from the original EVE event; with ECS the
            // sensor is the agent name, as elsewhere in EveBox.
            "host": "eve-host",
            "tls": {"version": "1.3", "client": {"server_name": "example.com"}},
            "suricata": {"eve": {
                "tls": {"sni": "example.com", "version": "TLS 1.3"},
                "stats": {"uptime": 10},
            }},
        });
        normalize(&mut event, true);
        assert_eq!(event["host"], "sensor-a");
        // Filled in where missing, never overwritten.
        assert_eq!(event["tls"]["sni"], "example.com");
        assert_eq!(event["tls"]["version"], "1.3");
        assert_eq!(event["tls"]["client"]["server_name"], "example.com");
        assert_eq!(event["stats"]["uptime"], 10);
    }

    #[test]
    fn sort_values_are_read_as_millis() {
        assert_eq!(sort_millis(&json!(1762560123456i64)), Some(1762560123456));
        assert_eq!(sort_millis(&json!(1762560123456.0)), Some(1762560123456));
        assert_eq!(sort_millis(&json!("1762560123456")), Some(1762560123456));
        assert_eq!(sort_millis(&json!(null)), None);
    }

    #[test]
    fn error_reasons_prefer_the_root_cause() {
        let error = json!({
            "root_cause": [{"type": "security_exception", "reason": "missing credentials"}],
            "type": "search_phase_execution_exception",
            "reason": "all shards failed",
        });
        assert_eq!(
            error_reason(&error),
            "security_exception: missing credentials"
        );
        assert_eq!(error_reason(&json!({"type": "x", "reason": "y"})), "x: y");
        assert_eq!(error_reason(&json!("plain")), "plain");
    }

    #[test]
    fn excerpts_are_short_and_whole_characters() {
        assert_eq!(excerpt("  Unauthorized \n"), "Unauthorized");
        let long = "é".repeat(300);
        let short = excerpt(&long);
        assert!(short.len() <= MAX_BODY_EXCERPT);
        assert!(short.chars().all(|c| c == 'é'));
    }
}
