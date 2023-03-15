// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::{filters::EveFilter, EveReader, Processor};
use crate::prelude::*;
use crate::{eve::filters::EveBoxMetadataFilter, importer::EventSink};
use std::time::Duration;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

/// Watches a collection of filename patterns and starts a new EVE
/// pipeline when a new file is found.
pub struct EvePatternWatcher {
    patterns: Vec<String>,
    filenames: HashSet<PathBuf>,
    sink: EventSink,
    filters: Vec<EveFilter>,
    end: bool,
    bookmark_directory: Option<String>,
    data_directory: Option<String>,
}

impl EvePatternWatcher {
    pub fn new(
        patterns: Vec<String>,
        sink: EventSink,
        filters: Vec<EveFilter>,
        end: bool,
        bookmark_directory: Option<String>,
        data_directory: Option<String>,
    ) -> Self {
        Self {
            patterns,
            filenames: HashSet::new(),
            sink,
            filters,
            end,
            bookmark_directory,
            data_directory,
        }
    }

    pub fn check(&mut self) {
        for pattern in &self.patterns {
            if let Ok(paths) = crate::path::expand(pattern) {
                for path in paths {
                    if !self.filenames.contains(&path) {
                        info!("Found EVE input file {}", path.display());
                        self.start_file(&path);
                        self.filenames.insert(path);
                    }
                }
            }
        }
    }

    fn start_file(&self, filename: &PathBuf) {
        let reader = EveReader::new(filename.clone());
        let mut processor = Processor::new(reader, self.sink.clone());
        let mut filters = self.filters.clone();
        filters.push(
            EveBoxMetadataFilter {
                filename: Some(filename.display().to_string()),
            }
            .into(),
        );

        let bookmark_filename = crate::server::main::get_bookmark_filename(
            filename,
            self.bookmark_directory.as_deref(),
            self.data_directory.as_deref(),
        );

        processor.filters = Arc::new(filters);
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
