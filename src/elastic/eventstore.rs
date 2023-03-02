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
use crate::datastore::{self, DatastoreError};
use crate::elastic::importer::Importer;
use crate::elastic::request::exists_filter;
use crate::elastic::{
    format_timestamp, request, AlertQueryOptions, ElasticResponse, ACTION_DEESCALATED,
    ACTION_ESCALATED, TAGS_ARCHIVED, TAGS_ESCALATED, TAG_ARCHIVED,
};
use crate::prelude::*;
use crate::querystring::parse_timestamp;
use crate::querystring::{self, Element, QueryString};
use crate::server::api;
use crate::server::session::Session;
use crate::util;
use crate::LOG_QUERIES;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

mod dhcp;
mod stats;

const MINIMUM_SHOULD_MATCH: &str = "minimum_should_match";

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
                "host" => "agent.name".to_string(),
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
                "event_type" => "event_type.keyword",
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

    fn apply_query_string(
        &self,
        q: &[querystring::Element],
        filter: &mut Vec<serde_json::Value>,
        should: &mut Vec<serde_json::Value>,
    ) {
        for el in q {
            match el {
                Element::String(s) => {
                    filter.push(query_string_query(s));
                }
                Element::KeyVal(key, val) => {
                    filter.push(request::term_filter(&self.map_field(key), val))
                }
                Element::Ip(ip) => {
                    should.push(json!({"term": {self.map_field("src_ip"): ip}}));
                    should.push(json!({"term": {self.map_field("dest_ip"): ip}}));
                    should.push(json!({"term": {self.map_field("dhcp.assigned_ip"): ip}}));
                    should.push(json!({"term": {self.map_field("dhcp.client_ip"): ip}}));
                    should.push(json!({"term": {self.map_field("dhcp.next_server_ip"): ip}}));
                    should.push(json!({"term": {self.map_field("dhcp.routers"): ip}}));
                    should.push(json!({"term": {self.map_field("dhcp.relay_ip"): ip}}));
                    should.push(json!({"term": {self.map_field("dhcp.subnet_mask"): ip}}));
                }
                Element::EarliestTimestamp(ts) => {
                    filter.push(request::timestamp_gte_filter(ts));
                }
                Element::LatestTimestamp(ts) => {
                    filter.push(request::timestamp_lte_filter(ts));
                }
            }
        }
    }

    pub fn build_inbox_query(&self, options: AlertQueryOptions) -> serde_json::Value {
        let mut filters = Vec::new();
        let mut should = Vec::new();

        // Set to true if the min timestamp is set in the query string
        let mut has_min_timestamp = false;

        if let Some(q) = &options.query_string {
            match crate::querystring::parse(q, None) {
                Ok(q) => {
                    self.apply_query_string(&q, &mut filters, &mut should);
                    has_min_timestamp = q
                        .iter()
                        .any(|e| matches!(&e, Element::EarliestTimestamp(_)));
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

        if !has_min_timestamp {
            if let Some(ts) = options.timestamp_gte {
                filters.push(json!({"range": {"@timestamp": {"gte": format_timestamp(ts)}}}));
            }
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

        if !should.is_empty() {
            query["query"]["bool"]["should"] = should.into();
            query["query"]["bool"][MINIMUM_SHOULD_MATCH] = 1.into();
        }

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
        let mut should = vec![];

        if let Some(event_type) = params.event_type {
            filters.push(request::term_filter(
                &self.map_field("event_type"),
                &event_type,
            ));
        }

        self.apply_query_string(&params.query_string_elements, &mut filters, &mut should);

        if let Some(timestamp) = params.min_timestamp {
            filters.push(request::timestamp_gte_filter(&timestamp));
        }

        if let Some(timestamp) = params.max_timestamp {
            filters.push(request::timestamp_lte_filter(&timestamp));
        }

        let sort_by = params.sort_by.unwrap_or_else(|| "@timestamp".to_string());
        let sort_order = params.order.unwrap_or_else(|| "desc".to_string());
        let size = params.size.unwrap_or(500);

        let mut body = json!({
            "query": {
                "bool": {
                    "filter": filters,
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

    async fn get_earliest_timestamp(&self) -> Result<Option<time::OffsetDateTime>, DatastoreError> {
        #[rustfmt::skip]
	let request = json!({
	    "query": {
		"bool": {
		    "filter": [
			{
			    "exists": {
				"field": self.map_field("event_type"),
			    },
			},
		    ],
		},
	    },
	    "sort": [{"@timestamp": {"order": "asc"}}],
	    "size": 1,
	});
        let response: serde_json::Value = self.search(&request).await?.json().await?;
        if let Some(hits) = response["hits"]["hits"].as_array() {
            for hit in hits {
                if let serde_json::Value::String(timestamp) = &hit["_source"]["@timestamp"] {
                    return Ok(Some(parse_timestamp(timestamp, None)?));
                }
            }
        }
        Ok(None)
    }

    pub(crate) async fn histogram_time(
        &self,
        interval: Option<u64>,
        q: &[querystring::Element],
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut filters = vec![exists_filter(&self.map_field("event_type"))];
        let mut should = vec![];
        self.apply_query_string(q, &mut filters, &mut should);

        let bound_max = time::OffsetDateTime::now_utc();
        let bound_min = if let Some(timestamp) = q.get_earliest() {
            timestamp
        } else if let Some(timestamp) = self.get_earliest_timestamp().await? {
            let earliest = timestamp;
            debug!(
                "No time-range provided by client, using earliest from database of {}",
                &earliest
            );
            earliest
        } else {
            warn!("Unable to determine earliest timestamp from Elasticsearch, assuming no events.");
            return Ok(vec![]);
        };

        let interval = if let Some(interval) = interval {
            interval
        } else {
            let range = bound_max.unix_timestamp() - bound_min.unix_timestamp();
            let interval = util::histogram_interval(range);
            debug!("No interval provided by client, using {interval}s");
            interval
        };

        #[rustfmt::skip]
        let request = json!({
            "query": {
		"bool": {
                    "filter": filters,
		},
            },
	    "size": 0,
            "sort":[{"@timestamp":{"order":"desc"}}],
	    "aggs": {
		"histogram": {
		    "date_histogram": {
			"field": "@timestamp",
			"fixed_interval": format!("{interval}s"),
			"min_doc_count": 0,
			"extended_bounds": {
			    "max": format_timestamp(bound_max),
			    "min": format_timestamp(bound_min),
			},
		    },
		},
	    },
        });

        let response: serde_json::Value = self.search(&request).await?.json().await?;
        let buckets = &response["aggregations"]["histogram"]["buckets"];
        let mut data = Vec::new();
        if let serde_json::Value::Array(buckets) = buckets {
            for bucket in buckets {
                data.push(json!({
                    "time": bucket["key"],
                    "count": bucket["doc_count"],
                }));
            }
        }

        Ok(data)
    }

    pub async fn group_by(
        &self,
        field: &str,
        size: usize,
        order: &str,
        q: Vec<querystring::Element>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut filter = vec![];
        let mut should = vec![];

        self.apply_query_string(&q, &mut filter, &mut should);

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

        filter.push(exists_filter(&self.map_field("event_type")));

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
            query["query"]["bool"][MINIMUM_SHOULD_MATCH] = 1.into();
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
