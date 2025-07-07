// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Eve processor. The combination of:
//! - reader: to read those eve log files
//! - importer: to send those events somewhere
//! - bookmarker: to remember the last location reader

use crate::prelude::*;

use crate::bookmark;
use crate::eve::reader::EveReader;
use crate::importer::EventSink;
use std::path::PathBuf;
use std::time::Duration;

use super::filters::EveFilterChain;

const DEFAULT_BATCH_SIZE: usize = 100;

pub(crate) struct Processor {
    pub reader: EveReader,
    pub importer: EventSink,
    pub filter_chain: Option<EveFilterChain>,
    pub bookmark_filename: Option<PathBuf>,
    pub report_interval: Duration,

    /// In the absence of a valid bookmark, should the reader start at the end of the file, or
    /// the beginning.
    pub end: bool,

    /// If in oneshot mode, will exit on EOF.
    pub oneshot: bool,
}

impl Processor {
    pub fn new(reader: EveReader, importer: EventSink) -> Self {
        Self {
            reader,
            importer,
            filter_chain: None,
            bookmark_filename: None,
            report_interval: Duration::from_secs(0),
            end: false,
            oneshot: false,
        }
    }

    /// Initialize the reader from a bookmark. Returns false if unable to initalize
    /// from the bookmark (invalid bookmark, file does not exist...).
    fn init_from_bookmark(&mut self) -> bool {
        if let Some(bookmark_filename) = &self.bookmark_filename {
            match bookmark::Bookmark::from_file(bookmark_filename) {
                Err(err) => {
                    warn!("Fail to load bookmark: {}", err);
                    false
                }
                Ok(bookmark) => {
                    if let Err(err) = bookmark.is_valid() {
                        info!("Invalid bookmark found: {}", err);
                        false
                    } else {
                        info!(
                            "Valid bookmark found, jumping to record: {}",
                            bookmark.offset
                        );
                        if let Err(err) = self.reader.goto_lineno(bookmark.offset) {
                            warn!("Failed to skip to line {}, error={}", bookmark.offset, err);
                            return false;
                        }
                        true
                    }
                }
            }
        } else {
            false
        }
    }

    pub async fn run(&mut self) {
        if !self.init_from_bookmark() && self.end {
            match self.reader.goto_end() {
                Ok(n) => {
                    info!("Skipped {} lines jumping to end of file", n);
                }
                Err(err) => {
                    error!("Failed to skip to end of file: {}", err);
                }
            }
        }
        let mut commits = 0;
        let mut count = 0;
        let mut eofs = 0;
        let mut last_report = std::time::Instant::now();
        loop {
            if self.report_interval > Duration::from_secs(0)
                && last_report.elapsed() > self.report_interval
            {
                debug!(filename = ?self.reader.filename, "count={}, commits={}, eofs={}", count, commits, eofs);
                count = 0;
                commits = 0;
                eofs = 0;
                last_report = std::time::Instant::now();
            }
            match self.reader.next_record() {
                Err(err) => {
                    error!(
                        "Failed to read event from {}: {}",
                        self.reader.filename.display(),
                        err
                    );
                    self.sleep_for(1000).await;
                }
                Ok(None) => {
                    eofs += 1;
                    if self.importer.pending() > 0 {
                        self.commit().await;
                        commits += 1;
                    } else if !self.oneshot && self.reader.is_file_changed() {
                        info!(
                            "File may have been rotated, will reopen: filename={:?}",
                            self.reader.filename
                        );
                        if let Err(err) = self.reader.reopen() {
                            error!("Failed to reopen {:?}, error={}", self.reader.filename, err);
                        }
                    }

                    if self.oneshot {
                        break;
                    }

                    // On EOF, always sleep for a second...
                    self.sleep_for(1000).await;
                }
                Ok(Some(mut event)) => {
                    if let Some(filters) = &self.filter_chain {
                        filters.run(&mut event);
                    }
                    count += 1;
                    let commit = self.importer.submit(event).await.unwrap();
                    if commit || self.importer.pending() >= DEFAULT_BATCH_SIZE {
                        self.commit().await;
                        commits += 1;
                    }
                }
            }

            // Always sleep for a minimal amount of time. If we have
            // the same number of processors is worker threads, and
            // we're processing a large backlog of events, we have to
            // give up some CPU to other tasks.
            tokio::task::yield_now().await;
        }
        info!(filename = ?self.reader.filename, "count={}, commits={}, eofs={}", count, commits, eofs);
    }

    async fn sleep_for(&self, millis: u64) {
        let d = std::time::Duration::from_millis(millis);
        tokio::time::sleep(d).await;
    }

    async fn commit(&mut self) {
        loop {
            match self.importer.commit().await {
                Ok(_n) => {
                    self.write_bookmark();
                    break;
                }
                Err(err) => {
                    error!("Failed to commit events (will try again): {}", err);
                    self.sleep_for(1000).await;
                }
            }
        }
    }

    fn write_bookmark(&mut self) {
        if let Some(bookmark_filename) = &self.bookmark_filename {
            if let Some(meta) = self.reader.metadata() {
                let bookmark = bookmark::Bookmark::from_metadata(&meta);
                if let Err(err) = bookmark.write(bookmark_filename) {
                    error!(
                        "Failed to write bookmark: filename={}, err={}",
                        bookmark_filename.display(),
                        err
                    );
                }
            }
        }
    }
}
