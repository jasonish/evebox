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
    root: Value,
}

impl<'a> Config<'a> {
    pub fn new(args: &'a clap::ArgMatches, filename: Option<&str>) -> anyhow::Result<Self> {
        let root = if let Some(filename) = filename {
            Self::load_file(filename)?
        } else {
            Value::Null
        };
        Ok(Self { args, root })
    }

    fn load_file(filename: &str) -> anyhow::Result<Value> {
        let input = std::fs::File::open(filename)?;
        Ok(serde_yaml::from_reader(&input)?)
    }

    /// Get a a value and deserialize into a type.
    ///
    /// This doesn't work for all types, for example booleans due to limitations
    /// with the Clap builder API.
    pub fn get<T>(&self, name: &str) -> anyhow::Result<Option<T>>
    where
        T: FromStr + DeserializeOwned + std::fmt::Debug + Sync + Send + Clone + 'static,
        <T as FromStr>::Err: Display,
    {
        let mut default_value: Option<T> = None;
        if let Ok(Some(value)) = self.args.try_get_one::<T>(name) {
            match self.args.value_source(name) {
                Some(ValueSource::CommandLine) | Some(ValueSource::EnvVariable) => {
                    return Ok(Some(value.clone()));
                }
                Some(ValueSource::DefaultValue) => {
                    default_value = Some(value.clone());
                }
                _ => {}
            }
        }

        // database.elasticsearch.url
        if name == "database.elasticsearch.url" {
            if let Ok(Some(v)) = self.get_env("ELASTICSEARCH_URL") {
                return Ok(Some(v));
            }
        }

        // Now the configuration file.
        if let Some(val) = self.get_node(&self.root, name) {
            return Ok(Some(serde_yaml::from_value(val.clone())?));
        }

        Ok(default_value)
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
        if let Ok(Some(value)) = self.args.try_get_one::<bool>(name) {
            return Ok(*value);
        }
        if let Some(val) = self.get_node(&self.root, name) {
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

    pub fn get_arg_strings(&self, name: &str) -> Option<Vec<String>> {
        if let Ok(Some(values)) = self.args.try_get_many::<String>(name) {
            let values: Vec<String> = values.map(|s| s.to_string()).collect();
            return Some(values);
        }
        return None;
    }

    /// NOTE: Only checks configuration file, not command line args.
    pub fn get_strings(&self, name: &str) -> anyhow::Result<Option<Vec<String>>> {
        self.get_config_value(name)
    }

    pub fn get_arg(&self, name: &str) -> Option<&str> {
        if let Ok(value) = self.args.try_get_one::<String>(name) {
            value.map(|s| &**s)
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

    /// Suppress clippy warning for another day...
    #[allow(clippy::only_used_in_recursion)]
    pub fn get_node(&self, root: &'a Value, name: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = name.splitn(2, '.').collect();
        let key = Value::String(parts[0].to_string());
        if let Value::Mapping(map) = root {
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
