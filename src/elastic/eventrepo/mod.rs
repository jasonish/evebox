// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use self::api::AlertGroupSpec;

use super::query_string_query;
use super::Client;
use super::HistoryEntry;
use super::HistoryEntryBuilder;
use super::TAGS_AUTO_ARCHIVED;
use super::TAG_ESCALATED;
use crate::datetime;
use crate::elastic::importer::ElasticEventSink;
use crate::elastic::request::exists_filter;
use crate::elastic::{request, ElasticResponse, TAGS_ARCHIVED, TAGS_ESCALATED, TAG_ARCHIVED};
use crate::prelude::*;
use crate::queryparser;
use crate::queryparser::QueryElement;
use crate::queryparser::QueryParser;
use crate::server::api;
use crate::server::session::Session;
use crate::util;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

mod alerts;
mod dhcp;
mod dns;
mod events;
mod stats;

const MINIMUM_SHOULD_MATCH: &str = "minimum_should_match";

/// Elasticsearch eventstore - for searching events.
#[derive(Debug, Clone)]
pub(crate) struct ElasticEventRepo {
    pub base_index: String,
    pub index_pattern: String,
    pub client: Client,
    pub ecs: bool,
    pub auto_archive_tx: Option<UnboundedSender<AlertGroupSpec>>,
}

impl ElasticEventRepo {
    pub fn start_archive_processor(&mut self) {
        let tx = super::autoarchive::AutoArchiveProcessor::start(self.clone());
        self.auto_archive_tx = Some(tx);
    }

    pub fn get_importer(&self) -> Option<ElasticEventSink> {
        if self.ecs {
            None
        } else {
            Some(super::importer::ElasticEventSink::new(
                self.client.clone(),
                &self.base_index,
                false,
            ))
        }
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
        // First resolve some quick shorthand.
        let name = match name {
            "@sid" => "alert.signature_id",
            "@sig" => "alert.signature",
            _ => name,
        };

        if self.ecs {
            match name {
                "dest_ip" => "destination.address".to_string(),
                "dest_port" => "destination.port".to_string(),
                "dns.rcode" => "dns.response_code".to_string(),
                "dns.rrname" => "dns.question.name".to_string(),
                "dns.rrtype" => "dns.question.type".to_string(),
                "dns.type" => name.to_string(),
                "host" => "agent.name".to_string(),
                "proto" => "network.transport".to_string(),
                "src_ip" => "source.address".to_string(),
                "src_port" => "source.port".to_string(),
                "timestamp" => "@timestamp".to_string(),
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
                "dns.type" => "dns.type.keyword",
                "dns.rcode" => "dns.rcode.keyword",
                "dns.rdata" => "dns.rdata.keyword",
                "dns.rrname" | "dns.queries.rrname" => "dns.queries.rrname.keyword",
                "dns.rrtype" => "dns.rrtype.keyword",
                "event_type" => "event_type.keyword",
                "host" => "host.keyword",
                "http.hostname" => "http.hostname.keyword",
                "http.http_user_agent" => "http.http_user_agent.keyword",
                "proto" => "proto.keyword",
                "src_ip" => "src_ip.keyword",
                "ssh.client.software_version" => "ssh.client.software_version.keyword",
                "ssh.server.software_version" => "ssh.server.software_version.keyword",
                "quic.sni" => "quic.sni.keyword",
                "quic.ja4" => "quic.ja4.keyword",
                "tls.issuerdn" => "tls.issuerdn.keyword",
                "tls.sni" => "tls.sni.keyword",
                "tls.subject" => "tls.subject.keyword",
                "tls.ja4" => "tls.ja4.keyword",
                "traffic.id" => "traffic.id.keyword",
                "traffic.label" => "traffic.label.keyword",
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
    ) -> Result<u64> {
        self.add_tags_by_query(query, &[tag], action).await
    }

    pub(crate) async fn add_tags_by_query(
        &self,
        query: serde_json::Value,
        tags: &[&str],
        action: &HistoryEntry,
    ) -> Result<u64> {
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
        let response = self.post(path, &body).await?.text().await?;
        let response: ElasticResponse = serde_json::from_str(&response)?;
        let updated = response.updated.unwrap_or_default();
        debug!("Tags added to {} events", updated);

        Ok(updated)
    }

    async fn remove_tag_by_query(
        &self,
        query: serde_json::Value,
        tag: &str,
        action: &HistoryEntry,
    ) -> Result<()> {
        self.remove_tags_by_query(query, &[tag], action).await
    }

    async fn remove_tags_by_query(
        &self,
        query: serde_json::Value,
        tags: &[&str],
        action: &HistoryEntry,
    ) -> Result<()> {
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
    ) -> Result<u64> {
        let mut must_not = Vec::new();
        for tag in tags {
            must_not.push(json!({"term": {"tags": tag}}));
        }

        let query = json!({
            "bool": {
                "filter": self.build_alert_group_filter(&alert_group, &mut must_not),
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
    ) -> Result<()> {
        let mut must_not = vec![];
        let mut filters = self.build_alert_group_filter(&alert_group, &mut must_not);
        for tag in tags {
            filters.push(json!({"term": {"tags": tag}}));
        }
        let query = json!({
            "bool": {
                "filter": filters,
                "must_not": must_not,
            }
        });
        self.remove_tags_by_query(query, tags, action).await
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<u64> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntryBuilder::new_archived().build();
        self.add_tag_by_query(query, TAG_ARCHIVED, &action).await
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<u64> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntryBuilder::new_escalate().build();
        self.add_tag_by_query(query, TAG_ESCALATED, &action).await
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<()> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntryBuilder::new_deescalate().build();
        self.remove_tag_by_query(query, TAG_ESCALATED, &action)
            .await
    }

    pub async fn comment_event_by_id(
        &self,
        event_id: &str,
        comment: String,
        session: Arc<Session>,
    ) -> Result<u64> {
        let query = json!({
            "bool": {
                "filter": {
                    "term": {"_id": event_id}
                }
            }
        });
        let action = HistoryEntryBuilder::new_comment()
            .username(session.username.clone())
            .comment(comment)
            .build();
        self.add_tags_by_query(query, &[], &action).await
    }

    pub async fn get_event_by_id(&self, event_id: String) -> Result<Option<serde_json::Value>> {
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
            bail!("elasticsearch: {}", error.first_reason());
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

    pub(crate) fn apply_query_string(
        &self,
        q: &[queryparser::QueryElement],
        filter: &mut Vec<serde_json::Value>,
        should: &mut Vec<serde_json::Value>,
        must_not: &mut Vec<serde_json::Value>,
    ) {
        for el in q {
            match &el.value {
                queryparser::QueryValue::String(v) => {
                    if el.negated {
                        must_not.push(query_string_query(v));
                    } else {
                        filter.push(query_string_query(v));
                    }
                }
                queryparser::QueryValue::KeyValue(k, v) => match k.as_ref() {
                    "@mac" => {
                        filter.push(json!({
                            "multi_match": {
                                "query": v,
                                "type": "most_fields",
                            }
                        }));
                    }
                    "@ip" => {
                        if el.negated {
                            must_not.push(json!({"term": {self.map_field("src_ip"): v}}));
                            must_not.push(json!({"term": {self.map_field("dest_ip"): v}}));
                            must_not.push(json!({"term": {self.map_field("dhcp.assigned_ip"): v}}));
                            must_not.push(json!({"term": {self.map_field("dhcp.client_ip"): v}}));
                            must_not
                                .push(json!({"term": {self.map_field("dhcp.next_server_ip"): v}}));
                            must_not.push(json!({"term": {self.map_field("dhcp.routers"): v}}));
                            must_not.push(json!({"term": {self.map_field("dhcp.relay_ip"): v}}));
                            must_not.push(json!({"term": {self.map_field("dhcp.subnet_mask"): v}}));
                        } else {
                            should.push(json!({"term": {self.map_field("src_ip"): v}}));
                            should.push(json!({"term": {self.map_field("dest_ip"): v}}));
                            should.push(json!({"term": {self.map_field("dhcp.assigned_ip"): v}}));
                            should.push(json!({"term": {self.map_field("dhcp.client_ip"): v}}));
                            should
                                .push(json!({"term": {self.map_field("dhcp.next_server_ip"): v}}));
                            should.push(json!({"term": {self.map_field("dhcp.routers"): v}}));
                            should.push(json!({"term": {self.map_field("dhcp.relay_ip"): v}}));
                            should.push(json!({"term": {self.map_field("dhcp.subnet_mask"): v}}));
                        }
                    }
                    "dns.type" => match v.as_ref() {
                        "query" | "request" => {
                            let bool_should = json!({
                                "bool": {
                                    "should": [
                                        {"term": {self.map_field(k): "query"}},
                                        {"term": {self.map_field(k): "request"}},
                                    ]
                                }
                            });
                            if el.negated {
                                must_not.push(bool_should);
                            } else {
                                filter.push(bool_should);
                            }
                        }
                        "answer" | "response" => {
                            let bool_should = json!({
                                "bool": {
                                    "should": [
                                        {"term": {self.map_field(k): "answer"}},
                                        {"term": {self.map_field(k): "response"}},
                                    ]
                                }
                            });
                            if el.negated {
                                must_not.push(bool_should);
                            } else {
                                filter.push(bool_should);
                            }
                        }
                        _ => {
                            if el.negated {
                                must_not.push(json!({"term": {self.map_field(k): v}}));
                            } else {
                                filter.push(json!({"term": {self.map_field(k): v}}));
                            }
                        }
                    },
                    _ => {
                        if k.starts_with(' ') {
                            warn!("Query parameter starting with a space: {k}");
                        }

                        let mapped_field = self.map_field(k);

                        if mapped_field.starts_with('@') {
                            warn!("Unhandled @ parameter in query string: {k}");
                        }

                        let expression = match mapped_field.as_ref() {
                            "dns.queries.rrname.keyword" => {
                                json!({
                                    "bool": {
                                        "should": [
                                            {"term": {"dns.queries.rrname.keyword": v}},
                                            {"term": {"dns.rrname.keyword": v}}
                                        ]
                                    }
                                })
                            }
                            _ => request::term_filter(&mapped_field, v),
                        };

                        if el.negated {
                            must_not.push(expression);
                        } else {
                            filter.push(expression);
                        }
                    }
                },
                queryparser::QueryValue::From(ts) => {
                    filter.push(request::timestamp_gte_filter(ts));
                }
                queryparser::QueryValue::To(td) => {
                    filter.push(request::timestamp_lte_filter(td));
                }
                queryparser::QueryValue::After(ts) => {
                    filter.push(request::timestamp_gt_filter(ts));
                }
                queryparser::QueryValue::Before(td) => {
                    filter.push(request::timestamp_lt_filter(td));
                }
            }
        }
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

        // Copy ECS values into _source where EVE / EveBox expect
        // them.
        //
        // Is there a way to make a table for this instead of each one
        // individually?
        if event["_source"]["event_type"].is_null() {
            event["_source"]["event_type"] =
                event["_source"]["suricata"]["eve"]["event_type"].clone();
        }

        if event["_source"]["alert"].is_null() {
            event["_source"]["alert"] = event["_source"]["suricata"]["eve"]["alert"].clone();
        }

        if event["_source"]["timestamp"].is_null() {
            event["_source"]["timestamp"] = event["_source"]["@timestamp"].clone();
        }

        if event["_source"]["src_ip"].is_null() {
            event["_source"]["src_ip"] = event["_source"]["source"]["ip"].clone();
        }

        if event["_source"]["src_port"].is_null() {
            event["_source"]["src_port"] = event["_source"]["source"]["port"].clone();
        }

        if event["_source"]["dest_ip"].is_null() {
            event["_source"]["dest_ip"] = event["_source"]["destination"]["ip"].clone();
        }

        if event["_source"]["dest_port"].is_null() {
            event["_source"]["dest_port"] = event["_source"]["destination"]["port"].clone();
        }

        let source = &mut event["_source"];

        if source["flow"].is_null() {
            source["flow"] = json!({
                "age": source["suricata"]["eve"]["flow"]["age"],
                "bytes_toclient": source["destination"]["bytes"],
                "bytes_toserver": source["source"]["bytes"],
                "pkts_toclient": source["destination"]["packets"],
                "pkts_toserver": source["source"]["packets"],
            });
        }

        // Simple hoisting of an object in suricata.eve.quic to
        // directly under _source.
        for obj in ["quic", "anomaly"] {
            if !source["suricata"]["eve"][obj].is_null() {
                source[obj] = source["suricata"]["eve"][obj].clone();
            }
        }

        // Merge DNS suricata.eve.dns into top level dns.
        if !source["dns"].is_null() && !source["suricata"]["eve"]["dns"].is_null() {
            let mut dns = source["dns"].clone();
            if let Some(inner) = source["suricata"]["eve"]["dns"].as_object() {
                for (k, v) in inner {
                    dns[k] = v.clone();
                }
            }
            source["dns"] = dns;
        }
    }

    pub async fn archive_by_alert_group(&self, alert_group: api::AlertGroupSpec) -> Result<u64> {
        let action = HistoryEntryBuilder::new_archived().build();
        self.add_tags_by_alert_group(alert_group, &TAGS_ARCHIVED, &action)
            .await
    }

    pub async fn auto_archive_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<u64> {
        let action = HistoryEntryBuilder::new_archived().build();
        self.add_tags_by_alert_group(alert_group, &TAGS_AUTO_ARCHIVED, &action)
            .await
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<u64> {
        let action = HistoryEntryBuilder::new_escalate()
            .username(session.username.clone())
            .build();
        self.add_tags_by_alert_group(alert_group, &TAGS_ESCALATED, &action)
            .await
    }

    pub async fn deescalate_by_alert_group(&self, alert_group: api::AlertGroupSpec) -> Result<()> {
        let action = HistoryEntryBuilder::new_deescalate().build();
        self.remove_tags_by_alert_group(alert_group, &TAGS_ESCALATED, &action)
            .await
    }

    pub(crate) async fn earliest_timestamp(&self) -> Result<Option<crate::datetime::DateTime>> {
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
                    let dt = crate::datetime::parse(timestamp, None)?;
                    return Ok(Some(dt));
                }
            }
        }
        Ok(None)
    }

    pub(crate) async fn histogram_time(
        &self,
        interval: Option<u64>,
        query: &[QueryElement],
    ) -> Result<Vec<serde_json::Value>> {
        let qs = QueryParser::new(query.to_vec());
        let mut filters = vec![exists_filter(&self.map_field("event_type"))];
        let mut should = vec![];
        let mut must_not = vec![];
        self.apply_query_string(query, &mut filters, &mut should, &mut must_not);

        let bound_max = datetime::DateTime::now();
        let bound_min = if let Some(timestamp) = qs.first_from() {
            timestamp
        } else if let Some(timestamp) = self.earliest_timestamp().await? {
            debug!(
                "No time-range provided by client, using earliest from database of {}",
                &timestamp
            );
            timestamp
        } else {
            warn!("Unable to determine earliest timestamp from Elasticsearch, assuming no events.");
            return Ok(vec![]);
        };

        let interval = if let Some(interval) = interval {
            interval
        } else {
            let range = bound_max.to_seconds() - bound_min.to_seconds();
            let interval = util::histogram_interval(range);
            debug!("No interval provided by client, using {interval}s");
            interval
        };

        #[rustfmt::skip]
        let request = json!({
            "query": {
		"bool": {
                    "filter": filters,
                    "must_not": must_not,
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
			    "max": bound_max.to_elastic(),
			    "min": bound_min.to_elastic(),
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

    pub async fn agg(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<queryparser::QueryElement>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut filter = vec![];
        let mut should = vec![];
        let mut must_not = vec![];

        self.apply_query_string(&query, &mut filter, &mut should, &mut must_not);

        let field = self.map_field(field);

        let mut agg = json!({});

        match field.as_ref() {
            "dns.queries.rrname.keyword" => {
                agg["script"] = json!({
                    "source": r#"
                        if (doc.containsKey('dns.queries.rrname.keyword') && doc['dns.queries.rrname.keyword'].size() != 0) {
                            return doc['dns.queries.rrname.keyword'];
                        }
                        else if (doc.containsKey('dns.rrname.keyword') && doc['dns.rrname.keyword'].size() != 0) {
                            return doc['dns.rrname.keyword'];
                        }
                        "#,
                });
            }
            _ => {
                agg["field"] = field.clone().into();
            }
        }

        let agg = if order == "asc" {
            // Increase the max_doc_count, otherwise only
            // terms that appear once will be returned, but
            // we're after the least occurring, but those
            // numbers could still be high.
            agg["max_doc_count"] = 100.into();
            json!({
                "rare_terms": agg
            })
        } else {
            agg["size"] = size.into();
            json!({
                "terms": agg
            })
        };

        filter.push(exists_filter(&self.map_field("event_type")));

        #[rustfmt::skip]
        let mut query = json!({
            "query": {
		"bool": {
		    "filter": filter,
                    "must_not": must_not,
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
            bail!("elasticsearch: {:?}", error);
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

    fn build_alert_group_filter(
        &self,
        request: &api::AlertGroupSpec,
        must_not: &mut Vec<serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        let mut filter = Vec::new();
        filter.push(json!({"exists": {"field": self.map_field("event_type")}}));
        filter.push(json!({"term": {self.map_field("event_type"): "alert"}}));
        filter.push(json!({
            "range": {
                self.map_field("timestamp"): {
                    "gte": request.min_timestamp,
                    "lte": request.max_timestamp,
                }
            }
        }));
        if let Some(src_ip) = &request.src_ip {
            filter.push(json!({"term": {self.map_field("src_ip"): src_ip}}));
        } else {
            must_not.push(json!({"exists": {"field": "src_ip"}}));
        }
        if let Some(dest_ip) = &request.dest_ip {
            filter.push(json!({"term": {self.map_field("dest_ip"): dest_ip}}));
        } else {
            must_not.push(json!({"exists": {"field": "dest_ip"}}));
        }
        filter.push(json!({"term": {self.map_field("alert.signature_id"): request.signature_id}}));

        // If we have a sensor, restrict the query to a sensor.
        if let Some(sensor) = &request.sensor {
            filter.push(json!({"term": {self.map_field("host"): sensor}}));
        }
        filter
    }

    pub async fn get_sensors(&self) -> anyhow::Result<Vec<String>> {
        #[rustfmt::skip]
        let request = json!({
            "size": 0,
            "query": {
                "bool": {
                    "must": [
                        {
                            "range": {
                                self.map_field("timestamp"): {
                                    "gte": "now-24h/h",
                                }
                            }
                        },
                        {
                            "term": {
                                self.map_field("event_type"): "stats"
                            }
                        },
                        {
                            "exists": {
                                "field": self.map_field("host")
                            }
                        }
                    ]
                }
            },
            "aggs": {
                "sensors": {
                    "terms": {
                        "field": self.map_field("host"),
                    }
                },
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
        Ok(sensors)
    }

    pub async fn get_event_types(&self) -> anyhow::Result<Vec<String>> {
        #[rustfmt::skip]
        let request = json!({
            "size": 0,
            "query": {
                "bool": {
                    "must": [
                        {
                            "exists": {
                                "field": self.map_field("event_type")
                            }
                        }
                    ]
                }
            },
            "aggs": {
                "event_types": {
                    "terms": {
                        "field": self.map_field("event_type"),
                        "size": 100,
                    }
                },
            }
        });
        let mut response: serde_json::Value = self.search(&request).await?.json().await?;
        let buckets = response["aggregations"]["event_types"]["buckets"].take();

        #[derive(Deserialize, Debug)]
        struct Bucket {
            key: String,
        }

        let buckets: Vec<Bucket> = serde_json::from_value(buckets)?;
        let event_types: Vec<String> = buckets.iter().map(|b| b.key.to_string()).collect();
        Ok(event_types)
    }
}
