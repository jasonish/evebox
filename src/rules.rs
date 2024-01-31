// SPDX-FileCopyrightText: (C) 2022 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

struct Inner {
    map: HashMap<u64, String>,
    files: HashMap<PathBuf, i64>,
}

pub fn read_next_rule(input: &mut dyn BufRead) -> Result<Option<String>, std::io::Error> {
    let mut line = String::new();
    loop {
        let mut tmp = String::new();
        let n = input.read_line(&mut tmp)?;
        if n == 0 {
            return Ok(None);
        }

        let tmp = tmp.trim();

        if !tmp.ends_with('\\') {
            line.push_str(tmp);
            break;
        }

        line.push_str(&tmp[..tmp.len() - 1]);
    }
    Ok(Some(line))
}

impl Inner {
    fn load_path(&mut self, path: &Path) {
        if let Ok(file) = std::fs::File::open(path) {
            let mut reader = std::io::BufReader::new(file);
            while let Ok(Some(line)) = read_next_rule(&mut reader) {
                if let Some(rule) = parse_line(&line) {
                    self.map.insert(rule.0, rule.1);
                }
            }
        }
    }
}

pub struct RuleMap {
    paths: Vec<String>,
    inner: RwLock<Inner>,
}

impl RuleMap {
    fn new() -> Self {
        Self {
            paths: Vec::new(),
            inner: RwLock::new(Inner {
                map: HashMap::new(),
                files: HashMap::new(),
            }),
        }
    }

    fn count(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.map.len()
    }

    pub fn filenames(&self) -> Vec<PathBuf> {
        let inner = self.inner.read().unwrap();
        inner.files.keys().cloned().collect()
    }

    pub fn find_by_sid(&self, sid: u64) -> Option<String> {
        let inner = self.inner.read().unwrap();
        if let Some(rule) = inner.map.get(&sid) {
            return Some(rule.to_string());
        }
        None
    }

    pub fn rescan(&self) {
        for path in &self.paths.clone() {
            match glob::glob(path) {
                Err(err) => {
                    error!("Bad rule path: {}: {}", path, err);
                }
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Err(err) => {
                                error!("Globbing error loading rules: {}", err);
                            }
                            Ok(path) => match std::fs::metadata(&path) {
                                Err(err) => {
                                    error!("Failed to load metadata for file {:?}: {}", path, err);
                                }
                                Ok(meta) => {
                                    let mtime =
                                        filetime::FileTime::from_last_modification_time(&meta)
                                            .unix_seconds();
                                    let mut inner = self.inner.write().unwrap();
                                    let prev = inner.files.insert(path.clone(), mtime);
                                    if let Some(prev) = prev {
                                        if mtime > prev {
                                            info!("Reloading rules from {:?}", path);
                                            (*inner).load_path(&path);
                                        }
                                    } else {
                                        info!("Loading rules from file {:?}", path);
                                        (*inner).load_path(&path);
                                    }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

fn parse_line(line: &str) -> Option<(u64, String)> {
    let mut offset = 0;
    if line.starts_with('#') {
        offset = 1;
    }

    let original = &line[offset..];
    match suricatax_rule_parser::parse_elements(original) {
        Ok((_, elements)) => {
            for element in &elements {
                if let suricatax_rule_parser::Element::Sid(sid) = element {
                    return Some((*sid, original.to_string()));
                }
            }
        }
        Err(err) => {
            debug!("Failed to parse as rule: {:?}: {}", err, line);
        }
    }

    None
}

pub fn load_rules(filenames: &[String]) -> RuleMap {
    let mut map = RuleMap::new();

    for path in filenames {
        map.paths.push(path.to_string());
    }

    map.rescan();
    info!("Loaded {} rules", map.count());

    map
}

/// Watch the known rule files for changes.  This is a polling loop as the
/// notify crate, at least as of the pre-5.0 releases could use some work.
pub fn watch_rules(rulemap: Arc<RuleMap>) {
    tokio::task::spawn_blocking(move || {
        let mut last_modified = std::time::SystemTime::now();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(6));
            let mut reload = false;
            let filenames = rulemap.filenames();
            for filename in &filenames {
                if let Ok(metadata) = std::fs::metadata(filename) {
                    if let Ok(modified) = metadata.modified() {
                        if modified.gt(&last_modified) {
                            reload = true;
                            last_modified = modified;
                        }
                    }
                }
            }
            if reload {
                info!("Rule modification detected, reloading");
                rulemap.rescan();
            }
        }
    });
}
