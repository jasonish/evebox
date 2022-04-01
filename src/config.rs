// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use clap::{ArgMatches, ValueSource};
use serde::de::DeserializeOwned;
use serde_yaml::Value;
use std::fmt::Display;
use std::str::FromStr;
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

    /// Get a a value and deserialize into a type.
    ///
    /// This doesn't work for all types, for example booleans due to limitations
    /// with the Clap builder API.
    pub fn get<T>(&self, name: &str) -> anyhow::Result<Option<T>>
    where
        T: FromStr + DeserializeOwned + std::fmt::Debug,
        <T as FromStr>::Err: Display,
    {
        // This will return the value if set on the command line, or in an environment
        // variable.
        if self.args.is_valid_arg(name)
            && (self.args.occurrences_of(name) > 0
                || (self.args.is_present(name)
                    && self.args.value_source(name) == Some(ValueSource::EnvVariable)))
        {
            return Ok(Some(self.args.value_of_t(name)?));
        }

        // database.elasticsearch.url
        match name {
            "database.elasticsearch.url" => {
                if let Ok(Some(v)) = self.get_env("ELASTICSEARCH_URL") {
                    return Ok(Some(v));
                }
            }
            _ => {}
        }

        // Now the configuration file.
        if let Some(val) = self.get_node(&self.root, name) {
            return Ok(Some(serde_yaml::from_value(val.clone())?));
        }

        // Maybe Clap as a default value.
        if self.args.is_valid_arg(name)
            && self.args.is_present(name)
            && self.args.value_source(name) == Some(ValueSource::DefaultValue)
        {
            return Ok(Some(self.args.value_of_t(name)?));
        }

        Ok(None)
    }

    pub fn get_env<T>(&self, name: &str) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        if let Ok(v) = std::env::var(name) {
            let value: serde_yaml::Value = serde_yaml::from_str(&v)?;
            return Ok(serde_yaml::from_value(value)?);
        }
        Ok(None)
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
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

    /// Return the configuration value as a boolean.
    ///
    /// If the value cannot be converted to a boolean an error will be returned. If the value
    /// is not found, false will be returned.
    pub fn get_bool(&self, name: &str) -> anyhow::Result<bool> {
        // This will catch the argument set on the command line or in the environment.
        if self.args.is_valid_arg(name) && self.args.is_present(name) {
            Ok(true)
        } else if let Some(val) = self.get_node(&self.root, name) {
            // Catch "yes" and "no".
            if let serde_yaml::Value::String(s) = val {
                if s == "yes" {
                    return Ok(true);
                } else if s == "no" {
                    return Ok(false);
                }
            }
            Ok(serde_yaml::from_value(val.clone())?)
        } else {
            Ok(false)
        }
    }

    pub fn get_present_arg(&self, name: &str) {
        if self.args.is_valid_arg(name) {
            dbg!(self.args.value_of(name));
            let values: Vec<&str> = self.args.values_of(name).unwrap().collect();
            dbg!(values);
        }
    }

    pub fn get_arg_strings(&self, name: &str) -> Option<Vec<String>> {
        if self.args.is_valid_arg(name) {
            if let Some(values) = self.args.values_of(name) {
                return Some(values.map(|s| s.to_string()).collect());
            }
        }
        None
    }

    pub fn get_strings(&self, name: &str) -> anyhow::Result<Option<Vec<String>>> {
        if self.args.is_valid_arg(name) && self.args.occurrences_of(name) > 0 {
            if let Some(strings) = self.args.values_of(name) {
                let strings: Vec<String> = strings.map(|s| s.to_string()).collect();
                return Ok(Some(strings));
            }
        }
        self.get_config_value(name)
    }

    pub fn get_arg(&self, name: &str) -> Option<&str> {
        if self.args.is_valid_arg(name) {
            self.args.value_of(name)
        } else {
            None
        }
    }

    pub fn get_value<T: DeserializeOwned>(&self, name: &str) -> anyhow::Result<Option<T>> {
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

    /// Get a configuration value explicitly from the configuration file.
    pub fn get_config_value<T: DeserializeOwned>(&self, name: &str) -> anyhow::Result<Option<T>> {
        if let Some(node) = self.get_node(&self.root, name) {
            Ok(Some(serde_yaml::from_value(node.clone())?))
        } else {
            Ok(None)
        }
    }
}
