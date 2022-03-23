// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_yaml;
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use tracing::error;

#[derive(Debug, Clone)]
enum UserFilterMatcher {
    Regex(RegularExpression),
    StartsWith(StartsWithMatcher),
    Exact(ExactMatcher),
}

impl UserFilterMatcher {
    pub fn is_match(&self, value: &JsonValue) -> bool {
        match self {
            Self::Regex(m) => m.is_match(value),
            Self::StartsWith(m) => m.is_match(value),
            Self::Exact(m) => m.is_match(value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExactMatcher {
    value: JsonValue,
}

impl ExactMatcher {
    pub fn new(value: JsonValue) -> Self {
        Self { value }
    }

    pub fn is_match(&self, value: &JsonValue) -> bool {
        value == &self.value
    }
}

#[derive(Debug, Clone)]
pub struct StartsWithMatcher {
    starts_with: String,
}

impl StartsWithMatcher {
    pub fn new(starts_with: String) -> Self {
        Self { starts_with }
    }

    pub fn is_match(&self, value: &JsonValue) -> bool {
        if let Some(s) = value.as_str() {
            if s.starts_with(&self.starts_with) {
                return true;
            }
        }
        false
    }
}

/// A regular expression matcher.
///
/// This matcher holds a collection of patterns and will match input that matches any
/// of the patterns.
#[derive(Debug, Clone)]
pub struct RegularExpression {
    patterns: Vec<regex::Regex>,
}

impl Default for RegularExpression {
    fn default() -> Self {
        Self::new()
    }
}

impl RegularExpression {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub fn add_pattern(&mut self, pattern: regex::Regex) -> &mut Self {
        self.patterns.push(pattern);
        self
    }

    pub fn is_match(&self, value: &JsonValue) -> bool {
        if let Some(s) = value.as_str() {
            for pattern in &self.patterns {
                if pattern.is_match(s) {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub enum UserFilterAction {
    #[serde(rename = "archive")]
    Archive,
}

#[derive(Debug)]
pub struct EveUserFilter {
    action: UserFilterAction,
    fields: Vec<(String, UserFilterMatcher)>,
}

impl EveUserFilter {
    pub fn new(action: UserFilterAction) -> Self {
        Self {
            action,
            fields: Vec::new(),
        }
    }

    pub fn is_match(&self, eve: &serde_json::Value) -> Option<UserFilterAction> {
        for (field, matcher) in &self.fields {
            if let Some(value) = self.get_value_for_field(field, eve) {
                if !matcher.is_match(value) {
                    return None;
                }
            }
        }
        Some(self.action.clone())
    }

    fn get_value_for_field<'a>(
        &self,
        field: &str,
        mut eve: &'a serde_json::Value,
    ) -> Option<&'a serde_json::Value> {
        for part in field.split('.') {
            if let Some(value) = eve.get(part) {
                eve = value;
            } else {
                return None;
            }
        }
        Some(eve)
    }

    fn add_field(&mut self, field: String, matcher: UserFilterMatcher) {
        self.fields.push((field, matcher));
    }
}

fn yaml_val_to_json(v: &YamlValue) -> anyhow::Result<JsonValue> {
    let s = serde_yaml::to_string(v)?;
    let js: JsonValue = serde_yaml::from_str(&s)?;
    Ok(js)
}

fn build_filters(filter_configs: Vec<UserFilterConfig>) -> anyhow::Result<Vec<EveUserFilter>> {
    let mut filters = Vec::new();
    for config in filter_configs {
        let mut filter = EveUserFilter::new(config.action);
        for (field, matcher) in config.matchers {
            match matcher {
                UserFilterConfigMatchValue::Scalar(v) => match yaml_val_to_json(&v) {
                    Err(err) => {
                        error!(
                            "Failed to use value as match for {}: {:?}: error={:?}",
                            &field, &v, err
                        );
                        continue;
                    }
                    Ok(v) => {
                        let matcher = ExactMatcher::new(v);
                        filter.add_field(field.to_string(), UserFilterMatcher::Exact(matcher));
                    }
                },
                UserFilterConfigMatchValue::Object(matcher) => {
                    if let Some(re) = matcher.re {
                        let mut re_matcher = RegularExpression::new();
                        match re {
                            UserFilterConfigRegexValue::Single(pattern) => {
                                let pattern = regex::Regex::new(&pattern)?;
                                re_matcher.add_pattern(pattern);
                            }
                            UserFilterConfigRegexValue::List(patterns) => {
                                for pattern in &patterns {
                                    let pattern = regex::Regex::new(pattern)?;
                                    re_matcher.add_pattern(pattern);
                                }
                            }
                        }
                        filter.add_field(field.to_string(), UserFilterMatcher::Regex(re_matcher));
                    }
                    if let Some(starts_with) = matcher.starts_with {
                        let starts_with = StartsWithMatcher::new(starts_with);
                        filter.add_field(
                            field.to_string(),
                            UserFilterMatcher::StartsWith(starts_with),
                        );
                    }
                }
            }
        }
        filters.push(filter);
    }
    Ok(filters)
}

pub fn from_str(s: &str) -> anyhow::Result<Vec<EveUserFilter>> {
    let filter_configs: Vec<UserFilterConfig> = serde_yaml::from_str(s)?;
    build_filters(filter_configs)
}

pub fn from_value(v: YamlValue) -> anyhow::Result<Vec<EveUserFilter>> {
    let filter_configs: Vec<UserFilterConfig> = serde_yaml::from_value(v)?;
    build_filters(filter_configs)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::logger::init_logger;
    use serde_json::json;

    #[test]
    fn test_regular_expression_matcher() {
        let mut re = RegularExpression::new();
        let pattern = regex::Regex::new("^ETN AGGRESSIVE").unwrap();
        re.add_pattern(pattern)
            .add_pattern(regex::Regex::new("^ETN TOR").unwrap());
        assert!(re.is_match(&json!("ETN AGGRESSIVE")));
        assert!(re.is_match(&json!("ETN AGGRESSIVE and some more data")));
        assert!(!re.is_match(&json!("ETN NOT-AGGRESSIVE")));
        assert!(re.is_match(&json!("ETN TOR")));
    }

    #[test]
    fn test_starts_with_matcher() {
        let matcher = StartsWithMatcher::new("ETN TOR".to_string());
        assert!(matcher.is_match(&json!("ETN TOR and some other stuff")));
        assert!(matcher.is_match(&json!("ETN TOR")));
        assert!(!matcher.is_match(&json!("ETN TO")));
        assert!(!matcher.is_match(&json!(1)));
    }

    #[test]
    fn test_from_str() {
        init_logger(tracing::Level::DEBUG);

        let yaml_string = r#"
  - action: archive
    match:
      alert.signature:
        re: ^ETN AGGRESSIVE
  - action: archive
    match:
      alert.signature:
        re: 
          - ^ETN AGGRESSIVE
          - ^ETN TOR
  - action: archive
    match:
      agent: firewall
        "#;
        let user_filter = from_str(yaml_string).unwrap();
        assert_eq!(user_filter.len(), 3);

        let event_aggressive = json!({
            "alert": {
                "signature": "ETN AGGRESSIVE test event",
            }
        });
        assert_eq!(
            user_filter[0].is_match(&event_aggressive),
            Some(UserFilterAction::Archive)
        );

        let event_tor = json!({
            "alert": {
                "signature": "ETN TOR test event",
            }
        });
        assert_eq!(
            user_filter[1].is_match(&event_aggressive),
            Some(UserFilterAction::Archive)
        );
        assert_eq!(
            user_filter[1].is_match(&event_tor),
            Some(UserFilterAction::Archive)
        );

        let event_other = json!({
            "alert": {
                "signature": "SURICATA_STREAM",
            },
            "agent": "firewall",
        });
        assert_eq!(user_filter[0].is_match(&event_other), None);
        assert_eq!(user_filter[1].is_match(&event_other), None);
        assert_eq!(
            user_filter[2].is_match(&event_other),
            Some(UserFilterAction::Archive)
        );
    }
}

#[derive(Debug, Deserialize)]
struct UserFilterConfig {
    pub action: UserFilterAction,
    #[serde(rename = "match")]
    pub matchers: HashMap<String, UserFilterConfigMatchValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UserFilterConfigMatchValue {
    Object(FilterMatch),
    Scalar(YamlValue),
}

#[derive(Debug, Deserialize)]
struct FilterMatch {
    re: Option<UserFilterConfigRegexValue>,
    starts_with: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UserFilterConfigRegexValue {
    Single(String),
    List(Vec<String>),
}
