// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use super::filters::EveFilterChain;
use super::spool::EveInput;
use super::{EveReader, Processor};
use crate::eve::filters::AddAgentFilenameFilter;
use crate::importer::EventSink;
use std::time::Duration;
use std::{collections::HashSet, path::PathBuf};

/// Watches a collection of filename patterns and starts a new EVE
/// pipeline when a new file is found.
pub(crate) struct EvePatternWatcher {
    patterns: Vec<String>,
    filenames: HashSet<PathBuf>,
    sink: EventSink,
    filters: EveFilterChain,
    end: bool,
    delete_processed_spool_files: bool,
    bookmark_directory: Option<String>,
    data_directory: Option<String>,
}

impl EvePatternWatcher {
    pub fn new(
        patterns: Vec<String>,
        sink: EventSink,
        filters: EveFilterChain,
        end: bool,
        delete_processed_spool_files: bool,
        bookmark_directory: Option<String>,
        data_directory: Option<String>,
    ) -> Self {
        Self {
            patterns,
            filenames: HashSet::new(),
            sink,
            filters,
            end,
            delete_processed_spool_files,
            bookmark_directory,
            data_directory,
        }
    }

    pub fn check(&mut self) {
        let mut paths = Vec::new();
        for pattern in &self.patterns {
            // This is for error reporting to the user, in the case
            // where the parent directory of the log files is not
            // readable by EveBox.
            if let Some(p) = PathBuf::from(pattern).parent()
                && let Err(err) = std::fs::read_dir(p)
            {
                warn!(
                    "Failed to read directory {}, EVE log files are likely unreadable: {}",
                    p.display(),
                    err
                );
            }
            if let Ok(found) = crate::path::expand(pattern) {
                paths.extend(found);
            }
        }

        for input in EveInput::group(paths, self.end) {
            let key = input.key().to_path_buf();
            if !self.filenames.contains(&key) {
                if input.is_spool() {
                    info!(
                        "Found EVE spool {} starting at {}",
                        key.display(),
                        input.path().display()
                    );
                } else {
                    info!("Found EVE input file {}", key.display());
                }
                self.start_file(&input);
                self.filenames.insert(key);
            }
        }
    }

    fn start_file(&self, input: &EveInput) {
        let filename = input.path();
        let input_key = input.key();
        let reader = EveReader::from_input(input);
        let mut processor = Processor::new(reader, self.sink.clone());
        let mut filters = self.filters.clone();
        filters.add_filter(AddAgentFilenameFilter::new(input_key.display().to_string()));

        let bookmark_filename = crate::server::main::get_bookmark_filename(
            input_key,
            self.bookmark_directory.as_deref(),
            self.data_directory.as_deref(),
        );

        processor.filter_chain = Some(filters);
        if bookmark_filename.is_none() && !self.end {
            warn!(
                "Failed to create bookmark file for {}, will read from end of file",
                filename.display()
            );
            processor.end = false;
        } else {
            processor.end = self.end;
        }
        processor.report_interval = Duration::from_secs(60);
        processor.bookmark_filename = bookmark_filename;
        if self.delete_processed_spool_files && input.is_spool() {
            if processor.bookmark_filename.is_some() {
                processor.set_delete_processed_spool_files(true);
            } else {
                warn!(
                    "Processed spool file deletion requires a bookmark for {}",
                    input_key.display()
                );
            }
        }
        info!("Starting EVE processor for {}", filename.display());
        tokio::spawn(async move {
            processor.run().await;
        });
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            loop {
                self.check();
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            }
        });
    }
}
