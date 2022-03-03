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
use serde::Deserialize;

/// Somewhat of an abstraction/combination of command line arguments and a
/// configuration file.
#[derive(Default, Clone, Debug)]
pub struct Settings {
    pub config: config::Config,
    pub args: clap::ArgMatches,
}

impl Settings {
    pub fn new(args: &clap::ArgMatches) -> Self {
        fix_deprecated_env_vars();
        let mut config = Settings {
            config: config::Config::default(),
            args: args.clone(),
        };
        config.load();
        config
    }

    fn load(&mut self) {
        let config_from_args = if self.args.is_valid_arg("config") {
            self.args
                .value_of("config")
                .map(|config| config.to_string())
        } else {
            None
        };

        if let Some(config) = config_from_args {
            self.merge_file(&config).unwrap();
        } else {
            // Check environment for configuration filename.
            if let Ok(config) = std::env::var("EVEBOX_CONFIG") {
                self.merge_file(&config).unwrap();
            }
        }

        // Migrate some old environment variables if found.
        if let Ok(val) = std::env::var("ELASTICSEARCH_URL") {
            if std::env::var("EVEBOX_DATABASE_ELASTICSEARCH_URL").is_err() {
                debug!(
                    "Setting EVEBOX_DATABASE_ELASTICSEARCH_URL to {} from ELASTICSEARCH_URL",
                    val
                );
                std::env::set_var("EVEBOX_DATABASE_ELASTICSEARCH_URL", val);
            }
        }

        self.config
            .merge(config::Environment::with_prefix("EVEBOX").separator("_"))
            .unwrap();
    }

    pub fn merge_file(&mut self, path: &str) -> Result<(), config::ConfigError> {
        let config_file = config::File::new(path, config::FileFormat::Yaml);
        self.config.merge(config_file)?;
        Ok(())
    }

    pub fn merge_yaml_str(&mut self, yaml: &str) -> Result<(), config::ConfigError> {
        let file = config::File::from_str(yaml, config::FileFormat::Yaml);
        self.config.merge(file)?;
        Ok(())
    }

    // There is a bit of a dance to get values in the following priority order...
    // - command line argument
    // - environment variable
    // - configuration file
    // - default set in clap
    pub fn get<'de, T: Deserialize<'de>>(&mut self, key: &str) -> Result<T, config::ConfigError> {
        if self.args.is_valid_arg(key) && self.args.occurrences_of(key) > 0 {
            self.set_from_args(key);
        }
        match self.config.get(key) {
            Ok(val) => Ok(val),
            Err(err) => {
                if let config::ConfigError::NotFound(_) = err {
                    self.set_from_args(key);
                    return self.config.get(key);
                }
                Err(err)
            }
        }
    }

    pub fn get_or_none<'de, T: Deserialize<'de>>(
        &mut self,
        key: &str,
    ) -> Result<Option<T>, config::ConfigError> {
        match self.get(key) {
            Ok(val) => Ok(Some(val)),
            Err(config::ConfigError::NotFound(_)) => Ok(None),
            Err(err) => return Err(err),
        }
    }

    pub fn get_bool(&self, key: &str) -> Result<bool, config::ConfigError> {
        if self.args.is_valid_arg(key) && self.args.occurrences_of(key) > 0 {
            return Ok(true);
        }
        match self.config.get_bool(key) {
            Ok(val) => Ok(val),
            Err(err) => {
                if let config::ConfigError::NotFound(_) = err {
                    Ok(false)
                } else {
                    return Err(err);
                }
            }
        }
    }

    pub fn count_of(&self, key: &str) -> u64 {
        if self.args.occurrences_of(key) > 0 {
            return self.args.occurrences_of(key);
        }
        if self.config.get::<config::Value>(key).is_ok() {
            return 1;
        }
        return 0;
    }

    fn set_from_args(&mut self, key: &str) {
        if self.args.is_valid_arg(key) && self.args.is_present(key) {
            self.config
                .set(key, self.args.value_of(key).unwrap())
                .unwrap();
        }
    }

    /// Return a value as an array of strings.
    pub fn get_string_array(&mut self, key: &str) -> Result<Vec<String>, config::ConfigError> {
        if self.args.occurrences_of(key) > 0 {
            let val = self
                .args
                .values_of(key)
                .unwrap()
                .map(String::from)
                .collect();
            return Ok(val);
        } else {
            let _: config::Value = self.config.get(key)?;
            if let Ok(val) = self.get::<Vec<String>>(key) {
                return Ok(val);
            }
            let val: String = self.get(key)?;
            return Ok(vec![val]);
        };
    }

    pub fn exists(&self, key: &str) -> bool {
        if self.args.value_of(key).is_some() {
            return true;
        }
        if self.config.get::<config::Value>(key).is_ok() {
            return true;
        }
        return false;
    }
}

fn fix_deprecated_env_vars() {
    let mut vars = std::collections::HashMap::new();
    vars.insert("EVEBOX_TLS_ENABLED", "EVEBOX_HTTP_TLS_ENABLED");
    vars.insert("EVEBOX_TLS_CERT", "EVEBOX_HTTP_TLS_CERTIFICATE");
    vars.insert("EVEBOX_TLS_KEY", "EVEBOX_HTTP_TLS_KEY");
    for (old, new) in vars {
        if let Ok(val) = std::env::var(old) {
            info!(
                "Found deprecated environment variable {}, setting {}",
                old, new
            );
            std::env::set_var(new, val);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use clap::*;

    const TEST_YAML: &str = r#"
verbose: true    
    "#;

    #[test]
    fn test_bool() {
        let args: &[&str] = &["test-args"];
        let parser =
            Command::new("EveBox").arg(Arg::new("verbose").short('v').multiple_occurrences(true));
        let matches = parser.get_matches_from(args);
        let settings = Settings::new(&matches);
        assert_eq!(settings.get_bool("verbose").unwrap(), false);
        assert_eq!(settings.count_of("verbose"), 0);

        let args: &[&str] = &["test-args", "-v"];
        let parser =
            Command::new("EveBox").arg(Arg::new("verbose").short('v').multiple_occurrences(true));
        let matches = parser.get_matches_from(args);
        let settings = Settings::new(&matches);
        assert_eq!(settings.get_bool("verbose").unwrap(), true);
        assert_eq!(settings.count_of("verbose"), 1);

        let args: &[&str] = &["test-args", "-v", "-v"];
        let parser =
            Command::new("EveBox").arg(Arg::new("verbose").short('v').multiple_occurrences(true));
        let matches = parser.get_matches_from(args);
        let settings = Settings::new(&matches);
        assert_eq!(settings.get_bool("verbose").unwrap(), true);
        assert_eq!(settings.count_of("verbose"), 2);

        let args: &[&str] = &["test-args"];
        let parser =
            Command::new("EveBox").arg(Arg::new("verbose").short('v').multiple_occurrences(true));
        let matches = parser.get_matches_from(args);
        let mut settings = Settings::new(&matches);
        settings.merge_yaml_str(TEST_YAML).unwrap();
        assert_eq!(settings.get_bool("verbose").unwrap(), true);
        assert_eq!(settings.count_of("verbose"), 1);
    }
}
