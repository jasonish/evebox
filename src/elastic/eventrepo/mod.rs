// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::Client;
use super::HistoryEntry;
use super::HistoryEntryBuilder;
use super::TAG_ESCALATED;
use super::TAGS_AUTO_ARCHIVED;
use super::query_string_query;
use crate::datetime;
use crate::elastic::fieldcaps::{self, FieldCaps, FieldKind};
use crate::elastic::importer::ElasticEventSink;
use crate::elastic::request::exists_filter;
use crate::elastic::{ElasticResponse, TAG_ARCHIVED, TAGS_ARCHIVED, TAGS_ESCALATED, request};
use crate::eventrepo::AlertGroupSpec;
use crate::prelude::*;
use crate::queryparser;
use crate::queryparser::QueryElement;
use crate::queryparser::QueryParser;
use crate::server::session::Session;
use crate::sqlite::configdb::{EventFilter, FilterOperator};
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
    base_index: String,
    index_pattern: String,
    client: Client,
    ecs: bool,
    opensearch: bool,
    auto_archive_tx: Option<UnboundedSender<EventFilter>>,
    field_caps: Arc<FieldCaps>,
}

impl ElasticEventRepo {
    pub fn new(
        base_index: String,
        index_pattern: String,
        client: Client,
        ecs: bool,
        opensearch: bool,
    ) -> Self {
        Self {
            base_index,
            index_pattern,
            client,
            ecs,
            opensearch,
            auto_archive_tx: None,
            field_caps: Arc::new(FieldCaps::default()),
        }
    }

    /// Load the field types of the event indices, and keep refreshing
    /// them in the background.
    pub fn start_field_caps_refresh(&self) {
        let repo = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(err) = repo
                    .field_caps
                    .refresh(&repo.client, &repo.index_pattern)
                    .await
                {
                    warn!("Failed to load Elasticsearch field capabilities: {err:#}");
                }
                tokio::time::sleep(fieldcaps::REFRESH_INTERVAL).await;
            }
        });
    }

    /// Reload the field types now, returning how many text fields have a
    /// keyword sub-field.
    pub(crate) async fn refresh_field_caps(&self) -> Result<usize> {
        self.field_caps
            .refresh(&self.client, &self.index_pattern)
            .await?;
        Ok(self.field_caps.keyword_count())
    }

    /// Make sure the field table knows the fields used by a query, plus
    /// any extra fields, refreshing it if needed.
    pub(crate) async fn ensure_fields(&self, query: &[QueryElement], extra: &[&str]) {
        let mut names: Vec<String> = extra.iter().map(|name| self.base_field(name)).collect();
        for element in query {
            if let queryparser::QueryValue::KeyValue(key, _) = &element.value
                && !key.starts_with('@')
            {
                names.push(self.base_field(key));
            }
        }
        if self.uses_dns_rrname(&names) {
            names.push("dns.rrname".to_string());
        }
        names.sort();
        names.dedup();
        self.field_caps
            .ensure(&self.client, &self.index_pattern, &names)
            .await;
    }

    fn uses_dns_rrname(&self, names: &[String]) -> bool {
        !self.ecs && names.iter().any(|name| name == "dns.queries.rrname")
    }

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
                self.opensearch,
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

    /// Resolve shorthand names, and for ECS the renamed fields, without
    /// choosing between a field and its keyword sub-field.
    fn base_field(&self, name: &str) -> String {
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
                // Suricata 8 moved the DNS query name; queries on it also
                // match the old location.
                "dns.rrname" => "dns.queries.rrname".to_string(),
                _ => name.to_string(),
            }
        }
    }

    /// Fields known to need their keyword sub-field in plain EVE indices,
    /// used until the field types are loaded, or for fields the indices
    /// don't have.
    fn fallback_keyword(&self, base: &str) -> Option<String> {
        if self.ecs {
            return None;
        }
        let known = matches!(
            base,
            "alert.category"
                | "alert.signature"
                | "anomaly.app_proto"
                | "anomaly.event"
                | "anomaly.layer"
                | "anomaly.type"
                | "app_proto"
                | "community_id"
                | "dest_ip"
                | "dhcp.assigned_ip"
                | "dhcp.client_mac"
                | "dns.type"
                | "dns.rcode"
                | "dns.rdata"
                | "dns.queries.rrname"
                | "dns.rrname"
                | "dns.rrtype"
                | "event_type"
                | "host"
                | "http.hostname"
                | "http.http_user_agent"
                | "proto"
                | "src_ip"
                | "ssh.client.software_version"
                | "ssh.server.software_version"
                | "quic.sni"
                | "quic.ja4"
                | "tls.issuerdn"
                | "tls.sni"
                | "tls.subject"
                | "tls.ja4"
                | "traffic.id"
                | "traffic.label"
                | "mdns.answers.rrname"
                | "mdns.queries.rrname"
        );
        known.then(|| format!("{base}.keyword"))
    }

    /// The field to aggregate, sort or exact-match on for a resolved
    /// field name: its keyword sub-field if it is analyzed text.
    fn exact_field(&self, base: &str) -> String {
        match self.field_caps.get(base) {
            Some(FieldKind::Text {
                keyword: Some(keyword),
            }) => keyword,
            Some(_) => base.to_string(),
            None => self
                .fallback_keyword(base)
                .unwrap_or_else(|| base.to_string()),
        }
    }

    /// Map a field name to the field to aggregate, sort or exact-match on.
    ///
    /// For ECS some of the field names have been changed completely. This
    /// happens when using Filebeat with the Suricata module.
    ///
    /// String fields are usually mapped as analyzed text with a keyword
    /// sub-field, which is looked up from the index field types.
    pub fn map_field(&self, name: &str) -> String {
        self.exact_field(&self.base_field(name))
    }

    /// A filter for a field having a value, as given in a query string.
    ///
    /// Matches the whole value exactly, so values with spaces, case or
    /// punctuation work. Text fields not on the fallback keyword list were
    /// queried with a term on the analyzed field before, which matched a
    /// lowercase word of the value, so that is still matched too.
    pub(crate) fn field_filter(&self, name: &str, value: &str) -> serde_json::Value {
        let base = self.base_field(name);
        // `text` is set when the field is known to be analyzed text.
        let either = |fields: &[&str], text: bool| {
            let mut should: Vec<serde_json::Value> = fields
                .iter()
                .map(|field| request::term_filter(field, value))
                .collect();
            // Long values are not indexed in the keyword sub-field
            // (ignore_above defaults to 256).
            if text && value.chars().count() > 256 {
                should.push(json!({"match_phrase": {&base: value}}));
            }
            json!({"bool": {"should": should, MINIMUM_SHOULD_MATCH: 1}})
        };
        match self.field_caps.get(&base) {
            Some(FieldKind::Text {
                keyword: Some(keyword),
            }) => {
                if self.fallback_keyword(&base).is_some() {
                    // Always matched exactly.
                    request::term_filter(&keyword, value)
                } else {
                    either(&[&keyword, &base], true)
                }
            }
            Some(_) => request::term_filter(&base, value),
            None => match self.fallback_keyword(&base) {
                Some(keyword) => request::term_filter(&keyword, value),
                None => either(&[&base, &format!("{base}.keyword")], false),
            },
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
            "source": "
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
            "source": "
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
        alert_group: AlertGroupSpec,
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
        alert_group: AlertGroupSpec,
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
        } else if let Some(hits) = &response.hits
            && let serde_json::Value::Array(hits) = &hits["hits"]
        {
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

        // If we get here something in the response was unexpected.
        warn!(
            "Received unexpected response for get_event_by_id from Elasticsearch: {:?}",
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
                    "host" => {
                        if v == "(no-name)" {
                            // Filter for documents without a host field
                            let expression = json!({"bool": {"must_not": {"exists": {"field": self.map_field("host")}}}});
                            if el.negated {
                                // If negated, we want documents WITH a host field
                                filter.push(json!({"exists": {"field": self.map_field("host")}}));
                            } else {
                                filter.push(expression);
                            }
                        } else {
                            let expression = json!({"term": {self.map_field("host"): v}});
                            if el.negated {
                                must_not.push(expression);
                            } else {
                                filter.push(expression);
                            }
                        }
                    }
                    _ => {
                        if k.starts_with(' ') {
                            warn!("Query parameter starting with a space: {k}");
                        }

                        let base = self.base_field(k);

                        if base.starts_with('@') {
                            warn!("Unhandled @ parameter in query string: {k}");
                        }

                        let expression = if self.uses_dns_rrname(std::slice::from_ref(&base)) {
                            json!({
                                "bool": {
                                    "should": [
                                        request::term_filter(&self.exact_field("dns.queries.rrname"), v),
                                        request::term_filter(&self.exact_field("dns.rrname"), v),
                                    ]
                                }
                            })
                        } else {
                            self.field_filter(k, v)
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
                queryparser::QueryValue::Archived => {
                    let expression = json!({"term": {"tags": TAG_ARCHIVED}});
                    if el.negated {
                        must_not.push(expression);
                    } else {
                        filter.push(expression);
                    }
                }
                queryparser::QueryValue::Escalated => {
                    let expression = json!({"term": {"tags": TAG_ESCALATED}});
                    if el.negated {
                        must_not.push(expression);
                    } else {
                        filter.push(expression);
                    }
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

        hoist_file_references(source);

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

    pub async fn archive_by_alert_group(&self, alert_group: AlertGroupSpec) -> Result<u64> {
        let action = HistoryEntryBuilder::new_archived().build();
        self.add_tags_by_alert_group(alert_group, &TAGS_ARCHIVED, &action)
            .await
    }

    fn build_auto_archive_query(&self, filter: &EventFilter) -> serde_json::Value {
        let mut filters = vec![
            json!({"exists": {"field": self.map_field("event_type")}}),
            json!({"term": {self.map_field("event_type"): "alert"}}),
        ];
        for condition in &filter.conditions {
            match condition.op {
                FilterOperator::Eq => filters.push(json!({
                    "term": {self.map_field(&condition.field): condition.value}
                })),
            }
        }
        let must_not: Vec<serde_json::Value> = TAGS_AUTO_ARCHIVED
            .iter()
            .map(|tag| json!({"term": {"tags": tag}}))
            .collect();
        json!({
            "bool": {
                "filter": filters,
                "must_not": must_not,
            }
        })
    }

    pub async fn auto_archive_by_filter(&self, filter: &EventFilter) -> Result<u64> {
        let query = self.build_auto_archive_query(filter);
        let action = HistoryEntryBuilder::new_auto_archived("filter-query").build();
        self.add_tags_by_query(query, &TAGS_AUTO_ARCHIVED, &action)
            .await
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<u64> {
        let action = HistoryEntryBuilder::new_escalate()
            .username(session.username.clone())
            .build();
        self.add_tags_by_alert_group(alert_group, &TAGS_ESCALATED, &action)
            .await
    }

    pub async fn deescalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<()> {
        let action = HistoryEntryBuilder::new_deescalate()
            .username(session.username.clone())
            .build();
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
        self.ensure_fields(query, &[]).await;
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
        self.ensure_fields(&query, &[field]).await;
        let request = self.agg_request(field, size, order, &query);
        let mut response: serde_json::Value = self.search(&request).await?.json().await?;

        // A field that appeared since the field types were last loaded,
        // within the refresh rate limit, is aggregated on as is, which
        // fails for text. Reload the field types and retry once.
        // With several indices, it can also fail on just some shards.
        let fielddata_error = |response: &serde_json::Value| {
            response["error"].to_string().contains("fielddata")
                || response["_shards"]["failures"]
                    .to_string()
                    .contains("fielddata")
        };
        if fielddata_error(&response) && self.field_caps.get(&self.base_field(field)).is_none() {
            let used = self.map_field(field);
            if let Err(err) = self.refresh_field_caps().await {
                warn!("Failed to refresh Elasticsearch field capabilities: {err:#}");
            } else if self.map_field(field) != used {
                let request = self.agg_request(field, size, order, &query);
                response = self.search(&request).await?.json().await?;
            }
        }

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

    fn agg_request(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: &[queryparser::QueryElement],
    ) -> serde_json::Value {
        let mut filter = vec![];
        let mut should = vec![];
        let mut must_not = vec![];

        self.apply_query_string(query, &mut filter, &mut should, &mut must_not);

        let base = self.base_field(field);
        let field = self.map_field(field);

        let mut agg = json!({});

        match base.as_ref() {
            "dns.queries.rrname" if !self.ecs => {
                agg["script"] = json!({
                    "source": r#"
                        if (doc.containsKey(params.queries) && doc[params.queries].size() != 0) {
                            return doc[params.queries];
                        }
                        else if (doc.containsKey(params.rrname) && doc[params.rrname].size() != 0) {
                            return doc[params.rrname];
                        }
                        "#,
                    "params": {
                        "queries": self.exact_field("dns.queries.rrname"),
                        "rrname": self.exact_field("dns.rrname"),
                    },
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
        let mut request = json!({
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
            request["query"]["bool"]["should"] = should.into();
            request["query"]["bool"][MINIMUM_SHOULD_MATCH] = 1.into();
        }

        request
    }

    fn build_alert_group_filter(
        &self,
        request: &AlertGroupSpec,
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

        if let Some(rrname) = &request.dns_rrname {
            filter.push(json!({
                "term": {self.map_field("dns.queries.rrname"): rrname}
            }));
        }
        if let Some(sni) = &request.tls_sni {
            filter.push(json!({"term": {self.map_field("tls.sni"): sni}}));
        }

        // If we have a sensor, restrict the query to a sensor.
        if let Some(sensor) = &request.sensor {
            if sensor == "(no-name)" {
                // Filter for documents without a host field
                filter.push(
                    json!({"bool": {"must_not": {"exists": {"field": self.map_field("host")}}}}),
                );
            } else {
                filter.push(json!({"term": {self.map_field("host"): sensor}}));
            }
        }
        filter
    }

    pub async fn get_sensors(&self) -> anyhow::Result<Vec<String>> {
        // First query: get sensors with host field
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
                        "size": 1000
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
        let mut sensors: Vec<String> = buckets.iter().map(|b| b.key.to_string()).collect();

        // Second query: check if there are any documents without a host field
        #[rustfmt::skip]
        let request_no_host = json!({
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
                        }
                    ],
                    "must_not": [
                        {
                            "exists": {
                                "field": self.map_field("host")
                            }
                        }
                    ]
                }
            }
        });
        let response_no_host: serde_json::Value =
            self.search(&request_no_host).await?.json().await?;

        // Handle both ES 6.x and 7.x+ response formats
        let total_no_host = if let Some(total_obj) = response_no_host["hits"]["total"].as_object() {
            // ES 7.x+ format: {"value": 123, "relation": "eq"}
            total_obj["value"].as_u64().unwrap_or(0)
        } else {
            // ES 6.x format: just a number
            response_no_host["hits"]["total"].as_u64().unwrap_or(0)
        };

        // If there are documents without a host field, add "(no-name)" to the list
        if total_no_host > 0 {
            sensors.push("(no-name)".to_string());
        }

        // Sort the sensors, keeping "(no-name)" at the end
        sensors.sort_by(|a, b| {
            if a == "(no-name)" {
                std::cmp::Ordering::Greater
            } else if b == "(no-name)" {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });

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

    /// Count the events matching a query string.
    pub(crate) async fn count(&self, query: &[QueryElement]) -> Result<u64> {
        self.ensure_fields(query, &[]).await;
        let mut filter = vec![exists_filter(&self.map_field("event_type"))];
        let mut should = vec![];
        let mut must_not = vec![];
        self.apply_query_string(query, &mut filter, &mut should, &mut must_not);

        let request = json!({
            "query": {
                "bool": {
                    "filter": filter,
                    "must_not": must_not,
                }
            },
            "size": 0,
            "track_total_hits": true,
        });

        let response: serde_json::Value = self.search(&request).await?.json().await?;
        response["hits"]["total"]
            .as_u64()
            .ok_or_else(|| anyhow!("Elasticsearch response had no field hits.total"))
    }

    pub fn get_base_index(&self) -> &str {
        &self.base_index
    }

    pub fn get_index_pattern(&self) -> &str {
        &self.index_pattern
    }

    pub fn get_client(&self) -> &Client {
        &self.client
    }

    pub fn is_ecs(&self) -> bool {
        self.ecs
    }

    pub fn is_opensearch(&self) -> bool {
        self.opensearch
    }
}

/// Hoist Suricata's file references from `suricata.eve` to directly under
/// `_source`, where file download expects them, unless already present
/// (for example from `event.original`).
fn hoist_file_references(source: &mut serde_json::Value) {
    for field in ["fileinfo", "files"] {
        if source[field].is_null() && !source["suricata"]["eve"][field].is_null() {
            source[field] = source["suricata"]["eve"][field].clone();
        }
    }
}

#[cfg(test)]
mod file_reference_test {
    use super::*;

    #[test]
    fn file_references_are_hoisted_from_suricata_eve() {
        let mut source = json!({
            "suricata": { "eve": {
                "fileinfo": { "sha256": "a".repeat(64) },
                "files": [ { "sha256": "b".repeat(64) } ],
            }},
        });
        hoist_file_references(&mut source);
        assert_eq!(source["fileinfo"]["sha256"], "a".repeat(64));
        assert_eq!(source["files"][0]["sha256"], "b".repeat(64));

        // Fields already present, e.g. from event.original, win.
        let mut source = json!({
            "fileinfo": { "sha256": "c".repeat(64) },
            "suricata": { "eve": { "fileinfo": { "sha256": "a".repeat(64) } } },
        });
        hoist_file_references(&mut source);
        assert_eq!(source["fileinfo"]["sha256"], "c".repeat(64));
        assert!(source["files"].is_null());
    }
}

#[cfg(test)]
mod field_mapping_test {
    use super::*;
    use std::collections::HashMap;

    fn repo(ecs: bool) -> ElasticEventRepo {
        ElasticEventRepo::new(
            "logstash".to_string(),
            "logstash-*".to_string(),
            Client::new("http://localhost:9200"),
            ecs,
            false,
        )
    }

    fn text(keyword: Option<&str>) -> FieldKind {
        FieldKind::Text {
            keyword: keyword.map(String::from),
        }
    }

    /// A repo whose field table is loaded with typical dynamic mappings.
    fn loaded_repo() -> ElasticEventRepo {
        let repo = repo(false);
        repo.field_caps.set(HashMap::from([
            ("tls.version".to_string(), text(Some("tls.version.keyword"))),
            ("tls.sni".to_string(), text(Some("tls.sni.keyword"))),
            ("http.url".to_string(), text(None)),
            ("flow.bytes_toserver".to_string(), FieldKind::Plain),
            // Mapped as keyword by a custom template.
            ("src_ip".to_string(), FieldKind::Plain),
            (
                "dns.queries.rrname".to_string(),
                text(Some("dns.queries.rrname.keyword")),
            ),
            ("dns.rrname".to_string(), text(Some("dns.rrname.keyword"))),
        ]));
        repo
    }

    #[test]
    fn map_field_without_field_types_uses_fallback_list() {
        let repo = repo(false);
        assert_eq!(repo.map_field("src_ip"), "src_ip.keyword");
        assert_eq!(repo.map_field("dns.rrname"), "dns.queries.rrname.keyword");
        assert_eq!(repo.map_field("@sig"), "alert.signature.keyword");
        assert_eq!(repo.map_field("tls.version"), "tls.version");
        assert_eq!(repo.map_field("alert.signature_id"), "alert.signature_id");
    }

    #[test]
    fn map_field_uses_field_types() {
        let repo = loaded_repo();
        assert_eq!(repo.map_field("tls.version"), "tls.version.keyword");
        assert_eq!(repo.map_field("tls.sni"), "tls.sni.keyword");
        assert_eq!(repo.map_field("http.url"), "http.url");
        assert_eq!(repo.map_field("flow.bytes_toserver"), "flow.bytes_toserver");
        // Keyword by mapping, so the fallback .keyword is not added.
        assert_eq!(repo.map_field("src_ip"), "src_ip");
        // Unknown fields still use the fallback list.
        assert_eq!(repo.map_field("dest_ip"), "dest_ip.keyword");
        assert_eq!(repo.map_field("flow.state"), "flow.state");
    }

    #[test]
    fn map_field_ecs() {
        let repo = repo(true);
        assert_eq!(repo.map_field("src_ip"), "source.address");
        assert_eq!(repo.map_field("tls.version"), "suricata.eve.tls.version");
        repo.field_caps.set(HashMap::from([(
            "suricata.eve.tls.version".to_string(),
            text(Some("suricata.eve.tls.version.keyword")),
        )]));
        assert_eq!(
            repo.map_field("tls.version"),
            "suricata.eve.tls.version.keyword"
        );
    }

    #[test]
    fn field_filter() {
        let repo = loaded_repo();

        // Exact on the keyword, plus the old term on the text field.
        assert_eq!(
            repo.field_filter("tls.version", "TLS 1.2"),
            json!({"bool": {"should": [
                {"term": {"tls.version.keyword": "TLS 1.2"}},
                {"term": {"tls.version": "TLS 1.2"}},
            ], "minimum_should_match": 1}})
        );

        // Fallback-list fields were always exact.
        assert_eq!(
            repo.field_filter("tls.sni", "example.com"),
            json!({"term": {"tls.sni.keyword": "example.com"}})
        );
        assert_eq!(
            repo.field_filter("dest_ip", "10.0.0.1"),
            json!({"term": {"dest_ip.keyword": "10.0.0.1"}})
        );

        // Not text, or text without a keyword sub-field: as before.
        assert_eq!(
            repo.field_filter("flow.bytes_toserver", "100"),
            json!({"term": {"flow.bytes_toserver": "100"}})
        );
        assert_eq!(
            repo.field_filter("http.url", "/index.html"),
            json!({"term": {"http.url": "/index.html"}})
        );

        // Unknown fields match either form.
        assert_eq!(
            repo.field_filter("flow.state", "closed"),
            json!({"bool": {"should": [
                {"term": {"flow.state": "closed"}},
                {"term": {"flow.state.keyword": "closed"}},
            ], "minimum_should_match": 1}})
        );
    }

    #[test]
    fn field_filter_long_value_matches_phrase() {
        let repo = loaded_repo();
        let value = "a ".repeat(200);
        let filter = repo.field_filter("tls.version", &value);
        assert_eq!(
            filter["bool"]["should"][2],
            json!({"match_phrase": {"tls.version": value}})
        );
    }

    #[test]
    fn dns_rrname_query_uses_both_names() {
        let repo = loaded_repo();
        let elements = queryparser::parse("dns.rrname:example.com", None).unwrap();
        let (mut filter, mut should, mut must_not) = (vec![], vec![], vec![]);
        repo.apply_query_string(&elements, &mut filter, &mut should, &mut must_not);
        assert_eq!(
            filter,
            vec![json!({"bool": {"should": [
                {"term": {"dns.queries.rrname.keyword": "example.com"}},
                {"term": {"dns.rrname.keyword": "example.com"}},
            ]}})]
        );
    }
}
