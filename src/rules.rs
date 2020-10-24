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

use std::collections::HashMap;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use suricata_rule_parser as parser;

use crate::logger::log;
use notify::{RecursiveMode, Watcher};

struct Inner {
    map: HashMap<u64, String>,
    files: HashMap<PathBuf, i64>,
}

impl Inner {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            files: HashMap::new(),
        }
    }

    fn load_path(&mut self, path: &PathBuf) {
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
            match glob::glob(&path) {
                Err(err) => {
                    log::error!("Bad rule path: {}: {}", path, err);
                }
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Err(err) => {
                                log::error!("Globbing error loading rules: {}", err);
                            }
                            Ok(path) => match std::fs::metadata(&path) {
                                Err(err) => {
                                    log::error!(
                                        "Failed to load metadata for file {:?}: {}",
                                        path,
                                        err
                                    );
                                }
                                Ok(meta) => {
                                    let mtime =
                                        filetime::FileTime::from_last_modification_time(&meta)
                                            .unix_seconds();
                                    let mut inner = self.inner.write().unwrap();
                                    let prev = (*inner).files.insert(path.clone(), mtime);
                                    if let Some(prev) = prev {
                                        if mtime > prev {
                                            log::info!("Reloading rules from {:?}", path);
                                            (*inner).load_path(&path);
                                        }
                                    } else {
                                        log::info!("Loading rules from file {:?}", path);
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
            log::trace!("Failed to parse as a rule ({}): {}", err, line);
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
    log::info!("Loaded {} rules", map.count());

    return map;
}

pub fn watch_rules(rulemap: Arc<RuleMap>) {
    tokio::spawn(async move {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::watcher(tx, std::time::Duration::from_secs(3)).unwrap();
        loop {
            let filenames = rulemap.filenames();
            for filename in filenames {
                watcher
                    .watch(filename.parent().unwrap(), RecursiveMode::NonRecursive)
                    .unwrap();
            }
            loop {
                if let Ok(event) = rx.recv() {
                    match event {
                        notify::DebouncedEvent::Write(_path)
                        | notify::DebouncedEvent::Create(_path) => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
            rulemap.rescan();
        }
    });
}
