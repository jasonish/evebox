// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use crate::elastic::HistoryEntryBuilder;
use crate::rules::RuleMap;
use crate::server::autoarchive::AutoArchive;
use crate::server::metrics::Metrics;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct EveFilterChain {
    filters: Vec<Arc<Box<dyn EveFilterTrait + Send + Sync>>>,
}

impl EveFilterChain {
    pub(crate) fn with_defaults() -> Self {
        let mut this = Self::default();
        this.add_filter(EnsureFilter::default());
        this.add_filter(AlertMetadataEveBoxActionFilter::default());
        this
    }

    pub(crate) fn add_filter<T>(&mut self, filter: T)
    where
        T: EveFilterTrait + Send + Sync + 'static,
    {
        let filter: Box<dyn EveFilterTrait + Send + Sync> = Box::new(filter);
        self.filters.push(Arc::new(filter));
    }

    pub(crate) fn run(&self, event: &mut serde_json::Value) {
        for filter in &self.filters {
            filter.run(event);
        }
    }
}

#[derive(Debug, Default, Clone)]
struct EnsureFilter {}

impl EveFilterTrait for EnsureFilter {
    fn run(&self, event: &mut serde_json::Value) {
        super::eve::ensure_has_history(event);
        super::eve::ensure_has_tags(event);
        super::eve::ensure_has_evebox(event);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GeoIpFilter {
    geoip: crate::geoip::GeoIP,
}

impl GeoIpFilter {
    pub(crate) fn new(geoip: crate::geoip::GeoIP) -> Self {
        Self { geoip }
    }
}

impl EveFilterTrait for GeoIpFilter {
    fn run(&self, event: &mut serde_json::Value) {
        self.geoip.add_geoip_to_eve(event);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AddAgentFilenameFilter {
    filename: serde_json::Value,
}

impl AddAgentFilenameFilter {
    pub(crate) fn new(filename: String) -> Self {
        Self {
            filename: serde_json::Value::String(filename),
        }
    }
}

impl EveFilterTrait for AddAgentFilenameFilter {
    fn run(&self, event: &mut serde_json::Value) {
        event["evebox"]["agent"]["filename"] = self.filename.clone();
    }
}

/// Filter to add the agent hostname. Should be used on the agent only.
#[derive(Clone, Debug)]
pub(crate) struct AddAgentHostnameFilter {
    hostname: serde_json::Value,
}

impl Default for AddAgentHostnameFilter {
    fn default() -> Self {
        let hostname = gethostname::gethostname().to_string_lossy().to_string();
        Self {
            hostname: hostname.into(),
        }
    }
}

impl EveFilterTrait for AddAgentHostnameFilter {
    fn run(&self, event: &mut serde_json::Value) {
        event["evebox"]["agent"]["hostname"] = self.hostname.clone();
    }
}

/// Filter to add the agent identifier, the exact name the agent claims on
/// the server's control channel. Should be used on the agent only.
#[derive(Clone, Debug)]
pub(crate) struct AddAgentIdFilter {
    agent_id: serde_json::Value,
}

impl AddAgentIdFilter {
    pub(crate) fn new(agent_id: String) -> Self {
        Self {
            agent_id: agent_id.into(),
        }
    }
}

impl EveFilterTrait for AddAgentIdFilter {
    fn run(&self, event: &mut serde_json::Value) {
        event["evebox"]["agent"]["id"] = self.agent_id.clone();
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AddFieldFilter {
    pub field: String,
    pub value: serde_json::Value,
}

impl AddFieldFilter {
    pub(crate) fn new<S: Into<String>>(field: S, value: serde_json::Value) -> Self {
        Self {
            field: field.into(),
            value,
        }
    }
}

impl EveFilterTrait for AddFieldFilter {
    fn run(&self, event: &mut serde_json::Value) {
        event[&self.field] = self.value.clone();
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AddRuleFilter {
    pub map: Arc<RuleMap>,
}

impl AddRuleFilter {
    pub(crate) fn new(map: Arc<RuleMap>) -> Self {
        Self { map }
    }
}

impl EveFilterTrait for AddRuleFilter {
    fn run(&self, event: &mut serde_json::Value) {
        if let serde_json::Value::String(_) = event["alert"]["rule"] {
            return;
        }
        if let Some(sid) = &event["alert"]["signature_id"].as_u64() {
            if let Some(rule) = self.map.find_by_sid(*sid) {
                event["alert"]["rule"] = rule.into();
            } else {
                trace!("Failed to find rule for SID {}", sid);
            }
        }
    }
}

/// Handle an action such as archive from
/// event["alert"]["metadata"]["evebox-action"] which may be set by
/// Suricata-Update.
#[derive(Clone, Default, Debug)]
struct AlertMetadataEveBoxActionFilter {}

impl EveFilterTrait for AlertMetadataEveBoxActionFilter {
    fn run(&self, event: &mut serde_json::Value) {
        if event.has_tag("evebox.archived") {
            // Just return, already archived.
            return;
        }

        let metadata_evebox_action = event["alert"]["metadata"]["evebox-action"].as_array();
        if let Some(action) = metadata_evebox_action
            && action.contains(&serde_json::Value::String("archive".into()))
        {
            mark_auto_archived(event, "metadata");
        }
    }
}

#[derive(Debug)]
pub(crate) struct AutoArchiveFilter {
    processor: Arc<RwLock<AutoArchive>>,
    metrics: Arc<Metrics>,
}

impl AutoArchiveFilter {
    pub(crate) fn new(auto_archive: Arc<RwLock<AutoArchive>>, metrics: Arc<Metrics>) -> Self {
        Self {
            processor: auto_archive,
            metrics,
        }
    }
}

impl EveFilterTrait for AutoArchiveFilter {
    fn run(&self, event: &mut serde_json::Value) {
        if event["event_type"] != "alert" || event.has_tag("evebox.archived") {
            return;
        }

        let processor = self.processor.read().unwrap();
        if processor.is_match(event) {
            mark_auto_archived(event, "filter");
            self.metrics.incr_autoarchived_by_filter(1)
        }
    }
}

fn mark_auto_archived(event: &mut serde_json::Value, cause: &str) {
    super::eve::ensure_has_tags(event);
    super::eve::ensure_has_evebox(event);
    super::eve::ensure_has_history(event);

    event["tags"]
        .as_array_mut()
        .unwrap()
        .extend(["evebox.archived".into(), "evebox.auto-archived".into()]);

    let history = HistoryEntryBuilder::new_auto_archived(cause).build();
    event["evebox"]["history"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(history));
}

pub(crate) trait EveFilterTrait: std::fmt::Debug {
    fn run(&self, event: &mut serde_json::Value);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sqlite::configdb::{
        EventFilter, FilterAction, FilterCondition, FilterEntry, FilterOperator,
    };

    fn assert_auto_archived_by(event: &serde_json::Value, cause: &str) {
        assert!(event.has_tag("evebox.archived"));
        assert!(event.has_tag("evebox.auto-archived"));

        let history = event["evebox"]["history"].as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["action"], "auto-archived");
        assert_eq!(history[0]["cause"], cause);
        assert!(history[0]["timestamp"].is_string());
    }

    #[test]
    fn test_clone() {
        let a = EveFilterChain::with_defaults();
        let mut b = a.clone();
        b.add_filter(AddAgentFilenameFilter::new("eve.json".to_string()));
        assert_eq!(a.filters.len(), b.filters.len() - 1);
    }

    #[test]
    fn test_add_agent_id_filter() {
        let filter = AddAgentIdFilter::new("edge-a".to_string());
        let mut event = serde_json::json!({ "event_type": "alert" });
        filter.run(&mut event);
        assert_eq!(event["evebox"]["agent"]["id"], "edge-a");
    }

    #[test]
    fn metadata_auto_archive_records_cause() {
        let filters = EveFilterChain::with_defaults();
        let mut event = serde_json::json!({
            "event_type": "alert",
            "alert": {
                "metadata": {
                    "evebox-action": ["archive"],
                },
            },
        });

        filters.run(&mut event);

        assert_auto_archived_by(&event, "metadata");
    }

    #[test]
    fn metadata_auto_archive_replaces_non_array_history() {
        let filters = EveFilterChain::with_defaults();
        let mut event = serde_json::json!({
            "event_type": "alert",
            "alert": {
                "metadata": {
                    "evebox-action": ["archive"],
                },
            },
            "evebox": {
                "history": "invalid",
            },
        });

        filters.run(&mut event);

        assert_auto_archived_by(&event, "metadata");
    }

    #[test]
    fn server_filter_auto_archive_records_cause() {
        let mut auto_archive = AutoArchive::default();
        auto_archive.add(&EventFilter::from(&FilterEntry {
            sensor: None,
            src_ip: None,
            dest_ip: None,
            dns_rrname: None,
            tls_sni: None,
            signature_id: 3301003,
            comment: None,
        }));

        let metrics = Arc::new(Metrics::default());
        let filter = AutoArchiveFilter::new(Arc::new(RwLock::new(auto_archive)), metrics);
        let mut event = serde_json::json!({
            "event_type": "alert",
            "alert": {
                "signature_id": 3301003,
            },
        });

        filter.run(&mut event);

        assert_auto_archived_by(&event, "filter");
    }

    #[test]
    fn server_filter_only_auto_archives_alerts() {
        let mut auto_archive = AutoArchive::default();
        auto_archive.add(&EventFilter {
            action: FilterAction::Archive,
            conditions: vec![FilterCondition {
                field: "src_ip".to_string(),
                op: FilterOperator::Eq,
                value: "10.1.1.1".into(),
            }],
        });

        let metrics = Arc::new(Metrics::default());
        let filter = AutoArchiveFilter::new(Arc::new(RwLock::new(auto_archive)), metrics);
        let mut flow = serde_json::json!({
            "event_type": "flow",
            "src_ip": "10.1.1.1",
        });
        filter.run(&mut flow);
        assert!(!flow.has_tag("evebox.archived"));

        let mut alert = serde_json::json!({
            "event_type": "alert",
            "src_ip": "10.1.1.1",
        });
        filter.run(&mut alert);
        assert_auto_archived_by(&alert, "filter");
    }
}
