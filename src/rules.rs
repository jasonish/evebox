// Copyright (C) 2022 Jason Ish
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
use std::collections::HashMap;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use suricata_rule_parser as parser;

struct Inner {
    map: HashMap<u64, String>,
    files: HashMap<PathBuf, i64>,
}

impl Inner {
    fn load_path(&mut self, path: &Path) {
        if let Ok(file) = std::fs::File::open(&path) {
            let mut reader = std::io::BufReader::new(file);
            while let Ok(Some(line)) = parser::read_next_rule(&mut reader) {
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
        (*inner).files.keys().cloned().collect()
    }

    pub fn find_by_sid(&self, sid: u64) -> Option<String> {
        let inner = self.inner.read().unwrap();
        if let Some(rule) = (*inner).map.get(&sid) {
            return Some(rule.to_string());
        }
        return None;
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
                                    let prev = (*inner).files.insert(path.clone(), mtime);
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
    match parser::parse_rule(&line[offset..]) {
        Ok(rule) => {
            if let Ok(Some(sid)) = parse_sid(&rule) {
                return Some((sid, rule.original));
            }
        }
        Err(err) => {
            trace!("Failed to parse as a rule ({}): {}", err, line);
        }
    }
    return None;
}

fn parse_sid(tokenized_rule: &parser::TokenizedRule) -> Result<Option<u64>, ParseIntError> {
    let mut sid: Option<u64> = None;

    for option in &tokenized_rule.options {
        if option.key == "sid" {
            let val = option.val.as_ref().unwrap().parse::<u64>()?;
            sid = Some(val);
        }
    }

    Ok(sid)
}

pub fn load_rules(filenames: &[String]) -> RuleMap {
    let mut map = RuleMap::new();

    for path in filenames {
        map.paths.push(path.to_string());
    }

    map.rescan();
    info!("Loaded {} rules", map.count());

    return map;
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