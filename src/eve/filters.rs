// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use crate::rules::RuleMap;
use crate::server::autoarchive::AutoArchive;
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

#[derive(Clone, Debug)]
pub(crate) struct AddFieldFilter {
    pub field: String,
    pub value: serde_json::Value,
}

impl AddFieldFilter {
    pub fn new<S: Into<String>>(field: S, value: serde_json::Value) -> Self {
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
    pub fn new(map: Arc<RuleMap>) -> Self {
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

        let metadata_evebox_action = event["alert"]["metadata"]["evebox-action"].as_array_mut();
        if let Some(action) = metadata_evebox_action {
            if action.contains(&serde_json::Value::String("archive".into())) {
                let tags = &mut event["tags"]
                    .as_array()
                    .cloned()
                    .unwrap_or_else(std::vec::Vec::new);
                tags.push("evebox.archived".into());
                tags.push("evebox.auto-archived".into());
                event["tags"] = serde_json::Value::Array(tags.clone());
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct AutoArchiveFilter {
    processor: Arc<RwLock<AutoArchive>>,
}

impl AutoArchiveFilter {
    pub(crate) fn new(auto_archive: Arc<RwLock<AutoArchive>>) -> Self {
        Self {
            processor: auto_archive,
        }
    }
}

impl EveFilterTrait for AutoArchiveFilter {
    fn run(&self, event: &mut serde_json::Value) {
        if event.has_tag("evebox.archived") {
            return;
        }

        let processor = self.processor.read().unwrap();
        if processor.is_match(event) {
            let tags = &mut event["tags"]
                .as_array()
                .cloned()
                .unwrap_or_else(std::vec::Vec::new);
            tags.push("evebox.archived".into());
            tags.push("evebox.auto-archived".into());
            event["tags"] = serde_json::Value::Array(tags.clone());
        }
    }
}

pub(crate) trait EveFilterTrait: std::fmt::Debug {
    fn run(&self, event: &mut serde_json::Value);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_clone() {
        let a = EveFilterChain::with_defaults();
        let mut b = a.clone();
        b.add_filter(AddAgentFilenameFilter::new("eve.json".to_string()));
        assert_eq!(a.filters.len(), b.filters.len() - 1);
    }
}
