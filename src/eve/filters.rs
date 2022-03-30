// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::eve::eve::EveJson;
use crate::prelude::*;
use crate::rules::RuleMap;

use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub enum EveFilter {
    GeoIP(crate::geoip::GeoIP),
    EveBoxMetadataFilter(EveBoxMetadataFilter),
    CustomFieldFilter(CustomFieldFilter),
    AddRuleFilter(AddRuleFilter),
    AutoArchiveFilter(AutoArchiveFilter),
    Filters(Arc<Vec<EveFilter>>),
}

impl EveFilter {
    pub fn run(&self, event: &mut EveJson) {
        match self {
            EveFilter::GeoIP(geoip) => {
                geoip.add_geoip_to_eve(event);
            }
            EveFilter::EveBoxMetadataFilter(filter) => {
                filter.run(event);
            }
            EveFilter::CustomFieldFilter(filter) => {
                filter.run(event);
            }
            EveFilter::AddRuleFilter(filter) => {
                filter.run(event);
            }
            EveFilter::Filters(filters) => {
                for filter in filters.iter() {
                    filter.run(event);
                }
            }
            EveFilter::AutoArchiveFilter(filter) => {
                filter.run(event);
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct EveBoxMetadataFilter {
    pub filename: Option<String>,
}

impl EveBoxMetadataFilter {
    pub fn run(&self, event: &mut EveJson) {
        // Create the "evebox" object.
        if let EveJson::Null = event["evebox"] {
            event["evebox"] = json!({});
        }

        // Add fields to the EveBox object.
        if let EveJson::Object(_) = &event["evebox"] {
            if let Some(filename) = &self.filename {
                event["evebox"]["filename"] = filename.to_string().into();
            }
        }

        // Add a tags object.
        if event.get("tags").is_none() {
            event["tags"] = serde_json::Value::Array(vec![]);
        }
    }
}

impl From<EveBoxMetadataFilter> for EveFilter {
    fn from(filter: EveBoxMetadataFilter) -> Self {
        EveFilter::EveBoxMetadataFilter(filter)
    }
}

#[derive(Clone)]
pub struct CustomFieldFilter {
    pub field: String,
    pub value: String,
}

impl CustomFieldFilter {
    pub fn new(field: &str, value: &str) -> Self {
        Self {
            field: field.to_string(),
            value: value.to_string(),
        }
    }

    pub fn run(&self, event: &mut EveJson) {
        event[&self.field] = self.value.clone().into();
    }
}

impl From<CustomFieldFilter> for EveFilter {
    fn from(filter: CustomFieldFilter) -> Self {
        EveFilter::CustomFieldFilter(filter)
    }
}

#[derive(Clone)]
pub struct AddRuleFilter {
    pub map: Arc<RuleMap>,
}

impl AddRuleFilter {
    pub fn run(&self, event: &mut EveJson) {
        if let EveJson::String(_) = event["alert"]["rule"] {
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

#[derive(Default, Clone, Debug)]
pub struct AutoArchiveFilter {}

impl AutoArchiveFilter {
    pub fn run(&self, event: &mut EveJson) {
        // Look for alert.metadata.
        let action = event["alert"]["metadata"]["evebox-action"]
            .as_array()
            .and_then(|a| a.iter().next().and_then(|e| e.as_str()));
        if let Some(action) = action {
            if action == "archive" {
                match &mut event["tags"] {
                    serde_json::Value::Array(tags) => {
                        tags.push("evebox.archived".into());
                        tags.push("evebox.auto-archived".into());
                    }
                    serde_json::Value::Null => {
                        event["tags"] = serde_json::Value::Array(vec![
                            "evebox.archived".into(),
                            "evebox.auto-archived".into(),
                        ]);
                    }
                    _ => {
                        warn!("Unable to auto-archive event, event has incompatible tags entry");
                    }
                }
            }
        }
    }
}
