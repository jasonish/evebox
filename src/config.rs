// SPDX-FileCopyrightText: (C) 2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::{parser::ValueSource, ArgMatches};
use serde::de::DeserializeOwned;
use serde_yaml::Value;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Clone)]
pub struct Config {
    pub args: ArgMatches,
    root: Value,
}

impl Config {
    pub fn new(args: clap::ArgMatches, filename: Option<&str>) -> anyhow::Result<Self> {
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

        // Check from environment variables. Clap does some of this in
        // command line parsing, but there may be cases we want to
        // respect environment variables, but don't go through command
        // line parsing.
        let environment_value = match name {
            "authentication.required" => Some("EVEBOX_AUTHENTICATION_REQUIRED"),
            _ => None,
        };
        if let Some(environment_value) = environment_value {
            if let Ok(Some(v)) = self.get_env(environment_value) {
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
        self.get(name).unwrap_or(None)
    }

    /// Return the configuration value as a boolean.
    ///
    /// If the value cannot be converted to a boolean an error will be returned. If the value
    /// is not found, false will be returned.
    pub fn get_bool(&self, name: &str) -> anyhow::Result<bool> {
        let val = self.get(name)?;
        Ok(val.unwrap_or(false))
    }

    /// Return a boolean configuration using a default value if the
    /// value is not found.
    pub fn get_bool_with_default(&self, name: &str, default: bool) -> bool {
        self.get(name).unwrap_or(Some(default)).unwrap_or(default)
    }

    pub fn get_arg_strings(&self, name: &str) -> Option<Vec<String>> {
        if let Ok(Some(values)) = self.args.try_get_many::<String>(name) {
            let values: Vec<String> = values.map(|s| s.to_string()).collect();
            return Some(values);
        }
        None
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
    pub fn get_node<'a>(&self, root: &'a Value, name: &str) -> Option<&'a Value> {
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

    /// Return an array of values, first from the command line then
    /// from the configuration file.
    pub fn get_many<T: DeserializeOwned + Sync + Send + Clone + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<Vec<T>>> {
        if let Some(values) = self.args.get_many::<T>(key) {
            let values: Vec<T> = values.cloned().collect();
            Ok(Some(values))
        } else {
            self.get_config_value(key)
        }
    }
}
