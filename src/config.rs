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

use crate::prelude::*;

pub struct Config {
    config: serde_yaml::Value,
    args: Option<clap::ArgMatches>,
}

impl Config {
    pub fn from_args(args: clap::ArgMatches, config_flag: Option<&str>) -> Result<Self> {
        let mut config = serde_yaml::Value::Null;
        if let Some(config_flag) = config_flag {
            if let Some(filename) = args.value_of(config_flag) {
                config = Self::load_file(filename)?;
            }
        }
        Ok(Self {
            config: config,
            args: Some(args),
        })
    }

    pub fn from_file(filename: &str) -> Result<Self> {
        let file = std::fs::File::open(filename)?;
        let config: serde_yaml::Value = serde_yaml::from_reader(file)?;
        Ok(Self {
            config: config,
            args: None,
        })
    }

    fn load_file(filename: &str) -> Result<serde_yaml::Value> {
        let file = std::fs::File::open(filename)?;
        let config: serde_yaml::Value = serde_yaml::from_reader(file)?;
        Ok(config)
    }

    pub fn env_key(&self, key: &str) -> String {
        let xform = key.replace(".", "_").replace("-", "_");
        format!("EVEBOX_{}", xform.to_uppercase())
    }

    pub fn get_string(&self, key: &str) -> Result<Option<String>> {
        let mut default: Option<String> = None;

        // First check if an argument was explicitly provided.
        if let Some(args) = &self.args {
            if args.occurrences_of(key) > 0 {
                return Ok(Some(args.value_of(key).unwrap().into()));
            }
            // Save the default...
            if args.is_present(key) {
                default = Some(args.value_of(key).unwrap().into());
            }
        }

        // Ok, no argument provided, check env.
        if let Ok(val) = std::env::var(self.env_key(key)) {
            return Ok(Some(val));
        }

        // No argument or environment variable, check the configuration file.
        let config_value = match self.find_value(key) {
            serde_yaml::Value::String(s) => Some(s.into()),
            serde_yaml::Value::Number(n) => Some(n.to_string()),
            _ => None,
        };
        if let Some(value) = config_value {
            return Ok(Some(value));
        }

        // Is there a default value configured with clap?
        return Ok(default);
    }

    pub fn get_strings(&self, key: &str) -> Result<Vec<String>> {
        if let Some(args) = &self.args {
            if args.occurrences_of(key) > 0 {
                let val = args.values_of(key).unwrap().map(String::from).collect();
                return Ok(val);
            }
        }

        match self.find_value(key) {
            serde_yaml::Value::Sequence(sequence) => {
                let mut vals: Vec<String> = Vec::new();
                for item in sequence {
                    if let Some(v) = item.as_str() {
                        vals.push(v.to_string());
                    }
                }
                return Ok(vals);
            }
            _ => Ok(Vec::new()),
        }
    }

    pub fn get_u64(&self, key: &str) -> Result<Option<u64>> {
        let mut default: Option<u64> = None;
        if let Some(args) = &self.args {
            if args.occurrences_of(key) > 0 {
                if let Some(v) = args.value_of(key) {
                    let v = v.parse::<u64>()?;
                    return Ok(Some(v));
                }
            }

            // Store the clap default value...
            if let Some(v) = args.value_of(key) {
                if let Ok(v) = v.parse::<u64>() {
                    default = Some(v);
                }
            }
        }

        match self.find_value(key) {
            serde_yaml::Value::Number(n) => {
                let v = n.as_u64().unwrap();
                Ok(Some(v))
            }
            serde_yaml::Value::Null => {
                // Return the clap default value.
                Ok(default)
            }
            _ => {
                bail!("value not convertable to string")
            }
        }
    }

    /// Get a value as a bool, returning false if the key does not exist.
    pub fn get_bool(&self, key: &str) -> Result<bool> {
        if let Some(args) = &self.args {
            if args.occurrences_of(key) > 0 {
                return Ok(true);
            }
        }

        // If no argument provided, check environment.
        if let Ok(val) = std::env::var(self.env_key(key)) {
            return match val.to_lowercase().as_ref() {
                "true" | "yes" | "1" => Ok(true),
                _ => Ok(false),
            };
        }

        if let serde_yaml::Value::Bool(v) = self.find_value(key) {
            return Ok(*v);
        }

        Ok(false)
    }

    fn find_value(&self, key: &str) -> &serde_yaml::Value {
        let val = &self.config[key];
        match val {
            serde_yaml::Value::Null => {}
            _ => return val,
        }
        let mut value = &self.config;
        for part in key.split('.') {
            value = &value[part];
        }
        return value;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_config() {
        let yaml = include_str!("test/server.yaml");
        let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let config = Config {
            config: v,
            args: None,
        };

        assert_eq!(
            config
                .get_string("database.elasticsearch.port")
                .unwrap()
                .unwrap(),
            "9200"
        );
        assert_eq!(
            config
                .get_string("database.elasticsearch.url")
                .unwrap()
                .unwrap(),
            "http://10.16.1.10:9200"
        );

        assert_eq!(config.get_bool("http.tls.enabled").unwrap(), true);
    }
}
