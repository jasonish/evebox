// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::query_string_query;
use super::Client;
use super::ElasticError;
use super::HistoryEntry;
use super::ACTION_ARCHIVED;
use super::ACTION_COMMENT;
use super::TAG_ESCALATED;
use crate::datastore::HistogramInterval;
use crate::datastore::{self, DatastoreError};
use crate::elastic::importer::Importer;
use crate::elastic::request::exists_filter;
use crate::elastic::request::range_gte_filter;
use crate::elastic::{
    format_timestamp, request, AlertQueryOptions, ElasticResponse, ACTION_DEESCALATED,
    ACTION_ESCALATED, TAGS_ARCHIVED, TAGS_ESCALATED, TAG_ARCHIVED,
};
use crate::prelude::*;
use crate::searchquery;
use crate::searchquery::Element;
use crate::server::api;
use crate::server::api::QueryStringParts;
use crate::server::session::Session;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

mod stats;

/// Elasticsearch eventstore - for searching events.
#[derive(Debug, Clone)]
pub struct EventStore {
    pub base_index: String,
    pub index_pattern: String,
    pub client: Client,
    pub ecs: bool,
    pub no_index_suffix: bool,
}

impl EventStore {
    pub fn get_importer(&self) -> Importer {
        super::importer::Importer::new(self.client.clone(), &self.base_index, false)
    }

    async fn post<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let path = format!("{}/{}", self.index_pattern, path);
        self.client.post(&path)?.json(body).send().await
    }

    pub async fn search<T: Serialize + std::fmt::Debug + ?Sized>(
        &self,
        body: &T,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let path = "_search?rest_total_hits_as_int=true&";
        self.post(path, body).await
    }

    /// Map field names as needed.
    ///
    /// For ECS some of the field names have been changed completely. This happens when using
    /// Filebeat with the Suricata module.
    ///
    /// For plain old Eve in Elasticsearch we still need to map some fields to their keyword
    /// variant.
    pub fn map_field(&self, name: &str) -> String {
        if self.ecs {
            match name {
                "dest_ip" => "destination.address".to_string(),
                "dest_port" => "destination.port".to_string(),
                "dns.rrname" => "dns.question.name".to_string(),
                "dns.rrtype" => "dns.question.type".to_string(),
                "dns.rcode" => "dns.response_code".to_string(),
                "dns.type" => name.to_string(),
                "src_ip" => "source.address".to_string(),
                "src_port" => "source.port".to_string(),
                _ => {
                    if name.starts_with("suricata") {
                        // Don't remap.
                        name.to_string()
                    } else {
                        format!("suricata.eve.{name}")
                    }
                }
            }
        } else {
            match name {
                "alert.category" => "alert.category.keyword",
                "alert.signature" => "alert.signature.keyword",
                "app_proto" => "app_proto.keyword",
                "community_id" => "community_id.keyword",
                "dest_ip" => "dest_ip.keyword",
                "dhcp.assigned_ip" => "dhcp.assigned_ip.keyword",
                "dhcp.client_mac" => "dhcp.client_mac.keyword",
                "dns.rrname" => "dns.rrname.keyword",
                "dns.rrtype" => "dns.rrtype.keyword",
                "dns.rcode" => "dns.rcode.keyword",
                "dns.rdata" => "dns.rdata.keyword",
                "host" => "host.keyword",
                "http.hostname" => "http.hostname.keyword",
                "http.http_user_agent" => "http.http_user_agent.keyword",
                "src_ip" => "src_ip.keyword",
                "ssh.client.software_version" => "ssh.client.software_version.keyword",
                "ssh.server.software_version" => "ssh.server.software_version.keyword",
                "traffic.id" => "traffic.id.keyword",
                "traffic.label" => "traffic.label.keyword",
                "tls.sni" => "tls.sni.keyword",
                "tls.subject" => "tls.subject.keyword",
                "tls.issuerdn" => "tls.issuerdn.keyword",
                _ => name,
            }
            .to_string()
        }
    }

    async fn add_tag_by_query(
        &self,
        query: serde_json::Value,
        tag: &str,
        action: &HistoryEntry,
    ) -> Result<(), DatastoreError> {
        self.add_tags_by_query(query, &[tag], action).await
    }

    async fn add_tags_by_query(
        &self,
        query: serde_json::Value,
        tags: &[&str],
        action: &HistoryEntry,
    ) -> Result<(), DatastoreError> {
        let script = json!({
            "lang": "painless",
            "inline": "
                if (params.tags != null) {
                    if (ctx._source.tags == null) {
                        ctx._source.tags = new ArrayList();
                    }
                    for (tag in params.tags) {
                        if (!ctx._source.tags.contains(tag)) {
                            ctx._source.tags.add(tag);
                        }
                    }
                }
                if (ctx._source.evebox == null) {
                    ctx._source.evebox = new HashMap();
                }
                if (ctx._source.evebox.history == null) {
                    ctx._source.evebox.history = new ArrayList();
                }
                ctx._source.evebox.history.add(params.action);
            ",
            "params": {
                "tags":   tags,
                "action": action,
            },
        });
        let body = json!({
            "query": query,
            "script": script,
        });

        let path = "_update_by_query?refresh=true&conflicts=proceed";
        let response: ElasticResponse = self.post(path, &body).await?.json().await?;
        let updated = if let Some(updated) = response.updated {
            updated
        } else {
            0
        };
        if updated == 0 {
            warn!(?response, "No events updated");
        }

        Ok(())
    }

    async fn remove_tag_by_query(
        &self,
        query: serde_json::Value,
        tag: &str,
        action: &HistoryEntry,
    ) -> Result<(), DatastoreError> {
        self.remove_tags_by_query(query, &[tag], action).await
    }

    async fn remove_tags_by_query(
        &self,
        query: serde_json::Value,
        tags: &[&str],
        action: &HistoryEntry,
    ) -> Result<(), DatastoreError> {
        let script = json!({
            "lang": "painless",
            "inline": "
                if (ctx._source.tags != null) {
                    for (tag in params.tags) {
                        ctx._source.tags.removeIf(entry -> entry == tag);
                    }
                }
                if (ctx._source.evebox == null) {
                    ctx._source.evebox = new HashMap();
                }
                if (ctx._source.evebox.history == null) {
                    ctx._source.evebox.history = new ArrayList();
                }
                ctx._source.evebox.history.add(params.action);
            ",
            "params": {
                "tags":   tags,
                "action": action,
            },
        });
        let body = json!({
            "query": query,
            "script": script,
        });
        let path = "_update_by_query?refresh=true&conflicts=proceed";
        let _reponse = self.post(path, &body).await?.bytes().await?;
        Ok(())
    }

    async fn add_tags_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        tags: &[&str],
        action: &HistoryEntry,
    ) -> Result<(), DatastoreError> {
        let mut must_not = Vec::new();
        for tag in tags {
            must_not.push(json!({"term": {"tags": tag}}));
        }

        let query = json!({
            "bool": {
                "filter": self.build_alert_group_filter(&alert_group),
                "must_not": must_not,
            }
        });

        self.add_tags_by_query(query, tags, action).await
    }

    async fn remove_tags_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        tags: &[&str],
        action: &HistoryEntry,
    ) -> Result<(), DatastoreError> {
        let mut filters = self.build_alert_group_filter(&alert_group);
        for tag in tags {
            filters.push(json!({"term": {"tags": tag}}));
        }
        let query = json!({
            "bool": {
                "filter": filters,
            }
        });
        self.remove_tags_by_query(query, tags, action).await
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntry {
            username: "anonymous".to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_ARCHIVED.to_string(),
            comment: None,
        };
        self.add_tag_by_query(query, TAG_ARCHIVED, &action).await
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntry {
            username: "anonymous".to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_ESCALATED.to_string(),
            comment: None,
        };
        self.add_tag_by_query(query, TAG_ESCALATED, &action).await
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntry {
            username: "anonymous".to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_DEESCALATED.to_string(),
            comment: None,
        };
        self.remove_tag_by_query(query, TAG_ESCALATED, &action)
            .await
    }

    pub async fn comment_event_by_id(
        &self,
        event_id: &str,
        comment: String,
        username: &str,
    ) -> Result<(), DatastoreError> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntry {
            username: username.to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_COMMENT.to_string(),
            comment: Some(comment),
        };
        self.add_tags_by_query(query, &[], &action).await
    }

    pub async fn get_event_by_id(
        &self,
        event_id: String,
    ) -> Result<Option<serde_json::Value>, DatastoreError> {
        let query = json!({
            "query": {
                "bool": {
                    "filter": {
                        "term": {"_id": event_id}
                    }
                }
            }
        });
        let response: ElasticResponse = self.search(&query).await?.json().await?;
        if let Some(error) = response.error {
            return Err(ElasticError::ErrorResponse(error.reason).into());
        } else if let Some(hits) = &response.hits {
            if let serde_json::Value::Array(hits) = &hits["hits"] {
                if !hits.is_empty() {
                    let mut hit = hits[0].clone();
                    if self.ecs {
                        self.transform_ecs(&mut hit);
                    }
                    return Ok(Some(hit));
                } else {
                    return Ok(None);
                }
            }
        }

        // If we get here something in the response was unexpected.
        warn!(
            "Received unexpected response for get_event_by_id from Elastic Search: {:?}",
            response
        );
        Ok(None)
    }

    fn query_string_to_filters(&self, query: &str) -> Vec<serde_json::Value> {
        let mut filters = Vec::new();
        match searchquery::parse(query) {
            Err(err) => {
                error!("Failed to parse query string: {} -- {}", &query, err);
            }
            Ok((_, elements)) => {
                for element in &elements {
                    filters.push(self.query_string_element_to_filter(element));
                }
            }
        }
        filters
    }

    fn process_query_string(
        &self,
        q: &QueryStringParts,
        filter: &mut Vec<serde_json::Value>,
        should: &mut Vec<serde_json::Value>,
    ) {
        for el in &q.elements {
            match el {
                Element::String(s) => {
                    filter.push(query_string_query(s));
                }
                Element::KeyVal(key, val) => match key.as_ref() {
                    "@before" => filter.push(request::range_lte_filter("@timestamp", val)),
                    "@after" => filter.push(request::range_gte_filter("@timestamp", val)),
                    "@ip" => {
                        should.push(json!({"term": {self.map_field("src_ip"): val}}));
                        should.push(json!({"term": {self.map_field("dest_ip"): val}}));
                    }
                    _ => filter.push(request::term_filter(&self.map_field(key), val)),
                },
            }
        }
    }

    fn query_string_element_to_filter(&self, el: &Element) -> serde_json::Value {
        match el {
            Element::KeyVal(key, val) => match key.as_ref() {
                "@before" => request::range_lte_filter("@timestamp", val),
                "@after" => request::range_gte_filter("@timestamp", val),
                _ => request::term_filter(&self.map_field(key), val),
            },
            Element::String(val) => query_string_query(val),
        }
    }

    pub fn build_inbox_query(&self, options: AlertQueryOptions) -> serde_json::Value {
        let mut filters = Vec::new();
        filters.push(json!({"exists": {"field": self.map_field("event_type")}}));
        filters.push(json!({"term": {self.map_field("event_type"): "alert"}}));
        if let Some(timestamp_gte) = options.timestamp_gte {
            filters
                .push(json!({"range": {"@timestamp": {"gte": format_timestamp(timestamp_gte)}}}));
        }
        if let Some(query_string) = options.query_string {
            filters.extend(self.query_string_to_filters(&query_string));
        }

        let mut must_not = Vec::new();
        for tag in options.tags {
            if let Some(tag) = tag.strip_prefix('-') {
                if tag == "archived" {
                    debug!("Rewriting tag {} to {}", tag, "evebox.archived");
                    must_not.push(json!({"term": {"tags": "evebox.archived"}}));
                } else {
                    let j = json!({"term": {"tags": tag}});
                    must_not.push(j);
                }
            } else if tag == "escalated" {
                debug!("Rewriting tag {} to {}", tag, "evebox.escalated");
                let j = json!({"term": {"tags": "evebox.escalated"}});
                filters.push(j);
            } else {
                let j = json!({"term": {"tags": tag}});
                filters.push(j);
            }
        }

        let query = json!({
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
                                    "terms": {"field": self.map_field("dest_ip"), "size": 500},
                                    "aggs": {
                                        "escalated": {"filter": {"term": {"tags": "evebox.escalated"}}},
                                        "newest": {"top_hits": {"size": 1, "sort": [{"@timestamp": {"order": "desc"}}]}},
                                        "oldest": {"top_hits": {"size": 1, "sort": [{"@timestamp": {"order": "asc"}}]}}
                                    },
                                },
                            },
                        },
                    },
                }
            }
        });

        query
    }

    pub async fn alerts(
        &self,
        options: AlertQueryOptions,
    ) -> Result<serde_json::Value, DatastoreError> {
        let query = self.build_inbox_query(options);
        let body = self.search(&query).await?.text().await?;
        let response: ElasticResponse = serde_json::from_str(&body)?;
        if let Some(error) = response.error {
            return Err(DatastoreError::ElasticSearchError(error.reason));
        }

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

        Ok(response)
    }

    fn transform_ecs(&self, event: &mut serde_json::Value) {
        let original_ecs = event.clone();
        // The "take" isn't really necessary but has the nice side affect that it removes
        // "original" from the result which makes for a better client side view of the event.
        if let Some(original) = event["_source"]["event"]["original"].take().as_str() {
            if let Ok(serde_json::Value::Object(m)) = serde_json::from_str(original) {
                for (k, v) in m {
                    event["_source"][k] = v;
                }
            }

            // Mainly for debugging ECS support, keep a copy of the ECS original record.
            event["ecs_original"] = original_ecs;
        }
    }

    pub async fn archive_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        let action = HistoryEntry {
            username: "anonymous".to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_ARCHIVED.to_string(),
            comment: None,
        };
        self.add_tags_by_alert_group(alert_group, &TAGS_ARCHIVED, &action)
            .await
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<(), DatastoreError> {
        let action = HistoryEntry {
            username: session.username().to_string(),
            //username: "anonymous".to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_ESCALATED.to_string(),
            comment: None,
        };
        self.add_tags_by_alert_group(alert_group, &TAGS_ESCALATED, &action)
            .await
    }

    pub async fn deescalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        let action = HistoryEntry {
            username: "anonymous".to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_DEESCALATED.to_string(),
            comment: None,
        };
        self.remove_tags_by_alert_group(alert_group, &TAGS_ESCALATED, &action)
            .await
    }

    pub async fn events(
        &self,
        params: datastore::EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        let mut filters = vec![request::exists_filter(&self.map_field("event_type"))];

        if let Some(event_type) = params.event_type {
            filters.push(request::term_filter(
                &self.map_field("event_type"),
                &event_type,
            ));
        }

        for element in &params.query_string_elements {
            filters.push(self.query_string_element_to_filter(element));
        }

        if let Some(timestamp) = params.min_timestamp {
            filters.push(request::timestamp_gte_filter(timestamp));
        }

        if let Some(timestamp) = params.max_timestamp {
            filters.push(request::timestamp_lte_filter(timestamp));
        }

        let sort_by = params.sort_by.unwrap_or_else(|| "@timestamp".to_string());
        let sort_order = params.order.unwrap_or_else(|| "desc".to_string());
        let size = params.size.unwrap_or(500);

        let body = json!({
            "query": {
                "bool": {
                    "filter": filters,
                }
            },
            "sort": [{sort_by: {"order": sort_order}}],
            "size": size,
        });

        let response: serde_json::Value = self.search(&body).await?.json().await?;
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

    pub async fn comment_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        comment: String,
        username: &str,
    ) -> Result<(), DatastoreError> {
        let entry = HistoryEntry {
            username: username.to_string(),
            timestamp: format_timestamp(time::OffsetDateTime::now_utc()),
            action: ACTION_COMMENT.to_string(),
            comment: Some(comment),
        };
        self.add_tags_by_alert_group(alert_group, &[], &entry).await
    }

    pub async fn histogram(
        &self,
        params: datastore::HistogramParameters,
    ) -> Result<serde_json::Value, DatastoreError> {
        let mut bound_max = None;
        let mut bound_min = None;
        let mut filters = vec![request::exists_filter(&self.map_field("event_type"))];
        if let Some(ts) = params.min_timestamp {
            filters.push(request::timestamp_gte_filter(ts));
            bound_min = Some(format_timestamp(ts));
        }
        if let Some(ts) = params.max_timestamp {
            filters.push(json!({"range":{"@timestamp":{"lte":format_timestamp(ts)}}}));
            bound_max = Some(format_timestamp(ts));
        }
        if let Some(event_type) = params.event_type {
            filters.push(request::term_filter(
                &self.map_field("event_type"),
                &event_type,
            ));
        }
        if let Some(dns_type) = params.dns_type {
            filters.push(request::term_filter(&self.map_field("dns.type"), &dns_type));
        }

        if let Some(query_string) = params.query_string {
            filters.extend(self.query_string_to_filters(&query_string));
        }

        if !self.ecs {
            if let Some(sensor_name) = params.sensor_name {
                if !sensor_name.is_empty() {
                    filters.push(request::term_filter(&self.map_field("host"), &sensor_name));
                }
            }
        }

        let mut should = Vec::new();
        let mut min_should_match = 0;
        if let Some(address_filter) = params.address_filter {
            should.push(request::term_filter(
                &self.map_field("src_ip"),
                &address_filter,
            ));
            should.push(request::term_filter(
                &self.map_field("dest_ip"),
                &address_filter,
            ));
            min_should_match = 1;
        }

        let interval = match params.interval {
            Some(interval) => match interval {
                HistogramInterval::Minute => "1m",
                HistogramInterval::Hour => "1h",
                HistogramInterval::Day => "1d",
            },
            None => "1h",
        };

        let major_version = match self.client.get_version().await {
            Ok(version) => version.major,
            Err(_) => 6,
        };
        let events_over_time = if major_version < 7 {
            json!({
                "date_histogram": {
                    "field": "@timestamp",
                    "interval": interval,
                    "min_doc_count": 0,
                    "extended_bounds": {
                        "max": bound_max,
                        "min": bound_min,
                    },
                }
            })
        } else {
            json!({
                "date_histogram": {
                    "field": "@timestamp",
                    "calendar_interval": interval,
                    "min_doc_count": 0,
                    "extended_bounds": {
                        "max": bound_max,
                        "min": bound_min,
                    },
                }
            })
        };

        let body = json!({
            "query": {
                "bool": {
                    "filter": filters,
                    "must_not": [{"term": {self.map_field("event_type"): "stats"}}],
                    "should": should,
                    "minimum_should_match": min_should_match,
                },
            },
            "size": 0,
            "sort":[{"@timestamp":{"order":"desc"}}],
            "aggs": {
                "events_over_time": events_over_time,
            }
        });

        let response: serde_json::Value = self.search(&body).await?.json().await?;
        let buckets = &response["aggregations"]["events_over_time"]["buckets"];
        let mut data = Vec::new();
        if let serde_json::Value::Array(buckets) = buckets {
            for bucket in buckets {
                data.push(json!({
                    "key": bucket["key"],
                    "count": bucket["doc_count"],
                    "key_as_string": bucket["key_as_string"],
                }));
            }
        }

        let response = json!({
            "data": data,
        });

        Ok(response)
    }

    pub async fn group_by(
        &self,
        field: &str,
        min_timestamp: time::OffsetDateTime,
        size: usize,
        order: &str,
        q: Option<QueryStringParts>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut filter = vec![];
        let mut should = vec![];

        if let Some(q) = &q {
            self.process_query_string(q, &mut filter, &mut should);
        }

        #[rustfmt::skip]
        let agg = if order == "asc" {
            // We're after a rare terms...
            json!({
		"rare_terms": {
                    "field": self.map_field(field),
                    // Increase the max_doc_count, otherwise only
                    // terms that appear once will be returned, but
                    // we're after the least occurring, but those
                    // numbers could still be high.
                    "max_doc_count": 100,
		}
            })
        } else {
            // This is a normal "Top 10"...
            json!({
		"terms": {
                    "field": self.map_field(field),
                    "size": size,
		},
            })
        };

        filter.push(exists_filter("event_type"));
        filter.push(range_gte_filter(
            "@timestamp",
            &format_timestamp(min_timestamp),
        ));

        #[rustfmt::skip]
        let mut query = json!({
            "query": {
		"bool": {
		    "filter": filter,
		},
            },
            // Not interested in individual documents, just the
            // aggregations on the filtered data.
            "size": 0,
            "aggs": {
		"agg": agg,
            },
        });

        if !should.is_empty() {
            query["query"]["bool"]["should"] = should.into();
            query["query"]["bool"]["minimum_should_match"] = 1.into();
        }

        let response: serde_json::Value = self.search(&query).await?.json().await?;

        if let Some(error) = response["error"].as_object() {
            error!("Elasticsearch \"group_by\" query failed: {error:?}");
            Err(DatastoreError::ElasticSearchError(format!("{error:?}")))
        } else {
            let mut data = vec![];
            if let serde_json::Value::Array(buckets) = &response["aggregations"]["agg"]["buckets"] {
                for bucket in buckets {
                    let entry = json!({
                        "key": bucket["key"],
                        "count": bucket["doc_count"],
                    });
                    data.push(entry);

                    // Elasticsearch doesn't take a size for rare terms,
                    // so stop when we've hit the requested size.
                    if data.len() == size {
                        break;
                    }
                }
            }
            Ok(data)
        }
    }

    pub async fn flow_histogram(
        &self,
        params: datastore::FlowHistogramParameters,
    ) -> Result<serde_json::Value, datastore::DatastoreError> {
        let mut filters = vec![
            request::term_filter(&self.map_field("event_type"), "flow"),
            request::exists_filter(&self.map_field("event_type")),
        ];
        if let Some(mints) = params.mints {
            filters.push(request::timestamp_gte_filter(mints));
        }
        if let Some(query_string) = params.query_string {
            filters.extend(self.query_string_to_filters(&query_string));
        }
        let query = json!({
            "query": {
                "bool": {
                    "filter": filters,
                }
            },
            "sort":[{"@timestamp":{"order":"desc"}}],
            "aggs": {
                "histogram": {
                    "aggs": {
                        "app_proto": {
                            "terms":{
                                "field": self.map_field("app_proto"),
                            }
                        }
                    },
                    "date_histogram": {
                        "field": "@timestamp",
                        "interval": params.interval,
                    }
                }
            }
        });
        let response: serde_json::Value = self.search(&query).await?.json().await?;
        let mut data = Vec::new();
        if let serde_json::Value::Array(buckets) = &response["aggregations"]["histogram"]["buckets"]
        {
            for bucket in buckets {
                let mut entry = json!({
                    "key": bucket["key"],
                    "events": bucket["doc_count"],
                });
                if let serde_json::Value::Array(buckets) = &bucket["app_proto"]["buckets"] {
                    let mut app_protos = json!({});
                    for bucket in buckets {
                        if let Some(key) = bucket["key"].as_str() {
                            if let Some(count) = bucket["doc_count"].as_u64() {
                                app_protos[key] = count.into();
                            }
                        }
                    }
                    entry["app_proto"] = app_protos;
                }
                data.push(entry);
            }
        }

        let response = json!({
            "data": data,
        });

        return Ok(response);
    }

    fn build_alert_group_filter(&self, request: &api::AlertGroupSpec) -> Vec<serde_json::Value> {
        let mut filter = Vec::new();
        filter.push(json!({"exists": {"field": self.map_field("event_type")}}));
        filter.push(json!({"term": {self.map_field("event_type"): "alert"}}));
        filter.push(json!({
            "range": {
                "@timestamp": {
                    "gte": request.min_timestamp,
                    "lte": request.max_timestamp,
                }
            }
        }));
        filter.push(json!({"term": {self.map_field("src_ip"): request.src_ip}}));
        filter.push(json!({"term": {self.map_field("dest_ip"): request.dest_ip}}));
        filter.push(json!({"term": {self.map_field("alert.signature_id"): request.signature_id}}));
        return filter;
    }

    pub async fn get_sensors(&self) -> anyhow::Result<Vec<String>> {
        let request = json!({
            "size": 0,
            "aggs": {
                "sensors": {
                    "terms": {
                        "field": self.map_field("host"),
                    }
                }
            }
        });
        let mut response: serde_json::Value = self.search(&request).await?.json().await?;
        let buckets = response["aggregations"]["sensors"]["buckets"].take();

        #[derive(Deserialize, Debug)]
        struct Bucket {
            key: String,
        }

        let buckets: Vec<Bucket> = serde_json::from_value(buckets)?;
        let sensors: Vec<String> = buckets.iter().map(|b| b.key.to_string()).collect();
        return Ok(sensors);
    }
}

fn elastic_format_interval(duration: time::Duration) -> String {
    let result = if duration < time::Duration::minutes(1) {
        format!("{}s", duration.whole_seconds())
    } else {
        format!("{}m", duration.whole_minutes())
    };
    debug!("Formatted duration of {:?} as {}", duration, &result);
    result
}
