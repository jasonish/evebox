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

//! Eve processor. The combination of:
//! - reader: to read those eve log files
//! - importer: to send those events somewhere
//! - bookmarker: to remember the last location reader

use crate::bookmark;
use crate::eve::filters::EveFilter;
use crate::eve::reader::EveReader;
use crate::importer::Importer;
use crate::logger::log;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_BATCH_SIZE: usize = 300;

pub struct Processor {
    pub reader: EveReader,
    pub importer: Importer,
    pub filters: Arc<Vec<EveFilter>>,
    pub bookmark_filename: Option<PathBuf>,
    pub report_interval: Duration,

    /// In the absence of a valid bookmark, should the reader start at the end of the file, or
    /// the beginning.
    pub end: bool,

    /// If in oneshot mode, will exit on EOF.
    pub oneshot: bool,

    pub batch_size: usize,
}

impl Processor {
    pub fn new(reader: EveReader, importer: Importer) -> Self {
        Self {
            reader: reader,
            importer: importer,
            filters: Arc::new(Vec::new()),
            bookmark_filename: None,
            report_interval: Duration::from_secs(0),
            end: false,
            oneshot: false,
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }

    /// Initialize the reader from a bookmark. Returns false if unable to initalize
    /// from the bookmark (invalid bookmark, file does not exist...).
    fn init_from_bookmark(&mut self) -> bool {
        if let Some(bookmark_filename) = &self.bookmark_filename {
            match bookmark::Bookmark::from_file(&bookmark_filename) {
                Err(err) => {
                    log::warn!("Fail to load bookmark: {}", err);
                    return false;
                }
                Ok(bookmark) => {
                    if let Err(err) = bookmark.is_valid() {
                        log::info!("Invalid bookmark found: {}", err);
                        return false;
                    } else {
                        log::info!(
                            "Valid bookmark found, jumping to record: {}",
                            bookmark.offset
                        );
                        if let Err(err) = self.reader.goto_lineno(bookmark.offset) {
                            log::warn!("Failed to skip to line {}, error={}", bookmark.offset, err);
                            return false;
                        }
                        return true;
                    }
                }
            }
        } else {
            return false;
        }
    }

    pub async fn run(&mut self) {
        if !self.init_from_bookmark() && self.end {
            match self.reader.goto_end() {
                Ok(n) => {
                    log::info!("Skipped {} lines jumping to end of file", n);
                }
                Err(err) => {
                    log::error!("Failed to skip to end of file: {}", err);
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
                log::debug!(filename = ?self.reader.filename, "count={}, commits={}, eofs={}", count, commits, eofs);
                count = 0;
                commits = 0;
                eofs = 0;
                last_report = std::time::Instant::now();
            }
            match self.reader.next_record() {
                Err(err) => {
                    log::error!("Failed to read event: {}", err);
                    self.sleep_for(1000).await;
                }
                Ok(None) => {
                    eofs += 1;
                    if self.importer.pending() > 0 {
                        self.commit().await;
                        commits += 1;
                    } else if !self.oneshot && self.reader.is_file_changed() {
                        log::info!(
                            "File may have been rotated, will reopen: filename={:?}",
                            self.reader.filename
                        );
                        if let Err(err) = self.reader.reopen() {
                            log::error!(
                                "Failed to reopen {:?}, error={}",
                                self.reader.filename,
                                err
                            );
                        }
                    }

                    if self.oneshot {
                        break;
                    }

                    // On EOF, always sleep for a second...
                    self.sleep_for(1000).await;
                }
                Ok(Some(mut event)) => {
                    for filter in &*self.filters {
                        filter.run(&mut event);
                    }
                    count += 1;
                    self.importer.submit(event).await.unwrap();
                    if self.importer.pending() >= 100 {
                        self.commit().await;
                        commits += 1;
                    }
                }
            }
        }
        log::info!(filename = ?self.reader.filename, "count={}, commits={}, eofs={}", count, commits, eofs);
    }

    async fn sleep_for(&self, millis: u64) {
        let d = std::time::Duration::from_millis(millis);
        tokio::time::delay_for(d).await;
    }

    async fn commit(&mut self) {
        loop {
            match self.importer.commit().await {
                Ok(_n) => {
                    self.write_bookmark();
                    break;
                }
                Err(err) => {
                    log::error!("Failed to commit events (will try again): {}", err);
                    self.sleep_for(1000).await;
                }
            }
        }
    }

    fn write_bookmark(&mut self) {
        if let Some(bookmark_filename) = &self.bookmark_filename {
            if let Some(meta) = self.reader.metadata() {
                let bookmark = bookmark::Bookmark::from_metadata(&meta);
                if let Err(err) = bookmark.write(&bookmark_filename) {
                    log::error!(
                        "Failed to write bookmark: filename={}, err={}",
                        bookmark_filename.display(),
                        err
                    );
                }
            }
        }
    }
}
