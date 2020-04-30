// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use crate::logger::log;
use serde::Deserialize;

/// Somewhat of an abstraction/combination of command line arguments and a
/// configuration file.
#[derive(Default, Clone, Debug)]
pub struct Settings {
    pub config: config::Config,
    pub args: clap::ArgMatches<'static>,
}

impl Settings {
    pub fn new(args: &clap::ArgMatches<'static>) -> Self {
        fix_deprecated_env_vars();
        let mut config = Settings {
            config: config::Config::default(),
            args: args.clone(),
        };
        config.load();
        config
    }

    fn load(&mut self) {
        if self.args.is_present("config") {
            if let Err(err) = self.config.merge(config::File::new(
                self.args.value_of("config").unwrap(),
                config::FileFormat::Yaml,
            )) {
                log::error!("Failed to load configuration file: {}", err);
                std::process::exit(1);
            }
        }

        self.config
            .merge(config::Environment::with_prefix("EVEBOX").separator("_"))
            .unwrap();
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
        if self.args.occurrences_of(key) > 0 {
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
        if self.args.occurrences_of(key) > 0 {
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
        if let Ok(_) = self.config.get::<config::Value>(key) {
            return 1;
        }
        return 0;
    }

    fn set_from_args(&mut self, key: &str) {
        if self.args.is_present(key) {
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
        if let Ok(_) = self.config.get::<config::Value>(key) {
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
            log::info!(
                "Found deprecated environment variable {}, setting {}",
                old,
                new
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
        let parser = App::new("EveBox").arg(Arg::with_name("verbose").short("v").multiple(true));
        let matches = parser.get_matches_from(args);
        let settings = Settings::new(&matches);
        assert_eq!(settings.get_bool("verbose").unwrap(), false);
        assert_eq!(settings.count_of("verbose"), 0);

        let args: &[&str] = &["test-args", "-v"];
        let parser = App::new("EveBox").arg(Arg::with_name("verbose").short("v").multiple(true));
        let matches = parser.get_matches_from(args);
        let settings = Settings::new(&matches);
        assert_eq!(settings.get_bool("verbose").unwrap(), true);
        assert_eq!(settings.count_of("verbose"), 1);

        let args: &[&str] = &["test-args", "-v", "-v"];
        let parser = App::new("EveBox").arg(Arg::with_name("verbose").short("v").multiple(true));
        let matches = parser.get_matches_from(args);
        let settings = Settings::new(&matches);
        assert_eq!(settings.get_bool("verbose").unwrap(), true);
        assert_eq!(settings.count_of("verbose"), 2);

        let args: &[&str] = &["test-args"];
        let parser = App::new("EveBox").arg(Arg::with_name("verbose").short("v").multiple(true));
        let matches = parser.get_matches_from(args);
        let mut settings = Settings::new(&matches);
        settings.merge_yaml_str(TEST_YAML).unwrap();
        assert_eq!(settings.get_bool("verbose").unwrap(), true);
        assert_eq!(settings.count_of("verbose"), 1);
    }
}
