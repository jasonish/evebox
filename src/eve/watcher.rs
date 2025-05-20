// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use super::filters::EveFilterChain;
#[cfg(unix)]
use super::EveReaderSocket;
use super::{EveReaderFile, Processor};
use crate::eve::filters::AddAgentFilenameFilter;
use crate::importer::EventSink;
use std::time::Duration;
use std::{collections::HashSet, path::PathBuf};

/// Watches a collection of filename patterns and starts a new EVE
/// pipeline when a new file is found.
pub(crate) struct EvePatternWatcher {
    patterns: Vec<String>,
    #[cfg(unix)]
    sockets: Vec<String>,
    filenames: HashSet<PathBuf>,
    sink: EventSink,
    filters: EveFilterChain,
    end: bool,
    bookmark_directory: Option<String>,
    data_directory: Option<String>,
}

impl EvePatternWatcher {
    pub fn new(
        patterns: Vec<String>,
        #[cfg(unix)] sockets: Vec<String>,
        sink: EventSink,
        filters: EveFilterChain,
        end: bool,
        bookmark_directory: Option<String>,
        data_directory: Option<String>,
    ) -> Self {
        Self {
            patterns,
            #[cfg(unix)]
            sockets,
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
            // This is for error reporting to the user, in the case
            // where the parent directory of the log files is not
            // readable by EveBox.
            if let Some(p) = PathBuf::from(pattern).parent() {
                if let Err(err) = std::fs::read_dir(p) {
                    warn!(
                        "Failed to read directory {}, EVE log files are likely unreadable: {}",
                        p.display(),
                        err
                    );
                }
            }
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
        #[cfg(unix)]
        for socket in &self.sockets {
            let path = PathBuf::from(socket);
            if !self.filenames.contains(&path) {
                info!("Starting EVE stream socket {}", path.display());
                if self.start_socket(path.clone()) {
                    self.filenames.insert(path);
                }
            }
        }
    }

    fn start_file(&self, filename: &PathBuf) {
        let reader = EveReaderFile::new(filename.clone());
        let mut processor = Processor::new(reader, self.sink.clone());
        let mut filters = self.filters.clone();
        filters.add_filter(AddAgentFilenameFilter::new(filename.display().to_string()));

        let bookmark_filename = crate::server::main::get_bookmark_filename(
            filename,
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
        info!("Starting EVE processor for {}", filename.display());
        tokio::spawn(async move {
            processor.run().await;
        });
    }

    #[cfg(unix)]
    fn start_socket(&self, filename: PathBuf) -> bool {
        let reader = match EveReaderSocket::new(filename.clone()) {
            Ok(socket) => socket,
            Err(err) => {
                warn!(
                    "Could not create socket file {}: {}",
                    filename.display(),
                    err
                );
                return false;
            }
        };
        let mut processor = Processor::new(reader, self.sink.clone());
        let mut filters = self.filters.clone();
        filters.add_filter(AddAgentFilenameFilter::new(filename.display().to_string()));

        processor.filter_chain = Some(filters);
        processor.report_interval = Duration::from_secs(60);
        info!("Starting EVE processor for {}", filename.display());
        tokio::spawn(async move {
            processor.run().await;
        });
        true
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
