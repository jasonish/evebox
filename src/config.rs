// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use clap::ArgMatches;
use serde::de::DeserializeOwned;
use serde_yaml::Value;
use tracing::debug;

pub struct Config<'a> {
    pub args: &'a ArgMatches,
    root: serde_yaml::Value,
}

impl<'a> Config<'a> {
    pub fn new(args: &'a clap::ArgMatches, filename: Option<&str>) -> anyhow::Result<Self> {
        let root = if let Some(filename) = filename {
            Self::load_file(filename)?
        } else {
            serde_yaml::Value::Null
        };
        Ok(Self { args, root })
    }

    fn load_file(filename: &str) -> anyhow::Result<serde_yaml::Value> {
        let input = std::fs::File::open(filename)?;
        Ok(serde_yaml::from_reader(&input)?)
    }

    pub fn get(&self, name: &str) -> Option<String> {
        if let Some(val) = self.get_arg(name) {
            debug!("Found {} in command line arguments: {}", name, val);
            Some(val.to_string())
        } else if let Some(val) = self.get_node(&self.root, name) {
            debug!("Found {} in configuration file: {:?}", name, val);
            Some(val.as_str().unwrap().to_string())
        } else {
            None
        }
    }

    pub fn get_bool(&self, name: &str) -> bool {
        if let Some(val) = self.get_arg(name) {
            debug!("Found {} in command line arguments: {}", name, val);
        }
        false
    }

    pub fn get_arg(&self, name: &str) -> Option<&str> {
        if self.args.is_valid_arg(name) {
            self.args.value_of(name)
        } else {
            None
        }
    }

    pub fn get_value<'de, T: DeserializeOwned>(&self, name: &str) -> anyhow::Result<Option<T>> {
        if let Some(value) = self.get_node(&self.root, name) {
            if let Value::Null = value {
                Ok(None)
            } else {
                Ok(Some(serde_yaml::from_value(value.clone())?))
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_value_as_array(&self, name: &str) -> Option<&'a serde_yaml::Value> {
        if let Some(_node) = self.get_node(&self.root, name) {}
        None
    }

    pub fn get_node(
        &self,
        root: &'a serde_yaml::Value,
        name: &str,
    ) -> Option<&'a serde_yaml::Value> {
        let parts: Vec<&str> = name.splitn(2, '.').collect();
        let key = serde_yaml::Value::String(parts[0].to_string());
        if let serde_yaml::Value::Mapping(map) = root {
            if let Some(value) = map.get(&key) {
                if parts.len() == 1 {
                    return Some(value);
                } else if value.is_mapping() {
                    return self.get_node(value, parts[1]);
                }
            }
        }
        None
    }
}
