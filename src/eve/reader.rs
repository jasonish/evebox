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

use crate::eve::eve::EveJson;
use crate::logger::log;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Seek;
use std::io::SeekFrom;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[derive(thiserror::Error, Debug)]
pub enum EveReaderError {
    #[error("failed to parse event")]
    ParseError(String),
    #[error("io error: {0}")]
    IoError(std::io::Error),
}

impl From<std::io::Error> for EveReaderError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

pub struct EveReader {
    pub filename: String,
    line: String,
    reader: Option<BufReader<std::fs::File>>,
    lineno: u64,
    offset: u64,
}

impl EveReader {
    pub fn new(filename: &str) -> Self {
        Self {
            filename: filename.to_string(),
            line: String::new(),
            reader: None,
            lineno: 0,
            offset: 0,
        }
    }

    pub fn open(&mut self) -> Result<(), EveReaderError> {
        let file = File::open(&self.filename)?;
        let reader = BufReader::new(file);
        self.reader = Some(reader);
        self.lineno = 0;
        self.offset = 0;
        Ok(())
    }

    pub fn reopen(&mut self) -> Result<(), EveReaderError> {
        if let Err(err) = self.open() {
            self.reader = None;
            self.lineno = 0;
            self.offset = 0;
            return Err(err);
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.reader.is_some()
    }

    pub fn goto_lineno(&mut self, lineno: u64) -> Result<u64, EveReaderError> {
        if self.reader.is_none() {
            self.open()?;
        }
        let mut count = 0;
        for _i in 0..lineno {
            if self.next_line()?.is_none() {
                break;
            }
            count += 1;
        }
        Ok(count)
    }

    pub fn goto_end(&mut self) -> Result<u64, EveReaderError> {
        if self.reader.is_none() {
            self.open()?;
        }
        loop {
            let line = self.next_line()?;
            if line.is_none() {
                break;
            }
        }

        Ok(self.lineno)
    }

    /// Return the current offset the reader is into the file.
    ///
    /// Will return 0 if no file is open.
    pub fn offset(&mut self) -> u64 {
        if let Some(reader) = &mut self.reader {
            if let Ok(pos) = reader.seek(SeekFrom::Current(0)) {
                return pos;
            }
        }
        return 0;
    }

    fn next_line(&mut self) -> Result<Option<&str>, EveReaderError> {
        self.line.truncate(0);
        if let Some(reader) = &mut self.reader {
            let pos = reader.seek(SeekFrom::Current(0))?;
            let n = reader.read_line(&mut self.line)?;
            if n > 0 {
                if !self.line.ends_with('\n') {
                    log::info!(
                        "Line does not end with new line character, seeking back to {}",
                        pos
                    );
                    reader.seek(SeekFrom::Start(pos))?;
                } else {
                    self.offset = pos + n as u64;
                    self.lineno += 1;
                    let line = self.line.trim();
                    return Ok(Some(line));
                }
            }
        }
        return Ok(None);
    }

    /// Not named next as we don't implement the iterator pattern (yet).
    pub fn next_record(&mut self) -> Result<Option<EveJson>, EveReaderError> {
        if self.reader.is_none() {
            self.open()?;
        }
        if self.reader.is_some() {
            let line = self.next_line()?;
            if let Some(line) = line {
                if !line.is_empty() {
                    let record: EveJson = serde_json::from_str(line).map_err(|err| {
                        log::error!("Failed to parse event: {}", err);
                        EveReaderError::ParseError(line.to_string())
                    })?;
                    return Ok(Some(record));
                }
            }
        }
        Ok(None)
    }

    pub fn metadata(&self) -> Option<Metadata> {
        if let Some(reader) = &self.reader {
            match reader.get_ref().metadata() {
                Err(err) => {
                    log::error!("Failed to get metadata for open reader: {}", err);
                    return None;
                }
                Ok(meta) => {
                    let metadata = Metadata {
                        filename: self.filename.clone(),
                        lineno: self.lineno,
                        size: meta.len(),
                        inode: self.inode(&meta),
                    };
                    return Some(metadata);
                }
            }
        }
        None
    }

    // An overly complex method to check if the file on disk has been truncate,
    // or replaced.
    pub fn is_file_changed(&self) -> bool {
        let open: Option<std::fs::Metadata> = if let Some(reader) = &self.reader {
            match reader.get_ref().metadata() {
                Err(err) => {
                    log::debug!("Failed to get metadata for open file: {}", err);
                    None
                }
                Ok(m) => Some(m),
            }
        } else {
            None
        };
        let disk: Option<std::fs::Metadata> = match std::fs::metadata(&self.filename) {
            Err(err) => {
                log::trace!("Failed to get metadata for file on disk: {}", err);
                None
            }
            Ok(m) => Some(m),
        };

        // If neither, then return false.
        if open.is_none() && disk.is_none() {
            log::trace!("open is none, disk is none -> false");
            return false;
        }

        // If we don't have an open file, but there is an on disk file, return true.
        if open.is_none() && disk.is_some() {
            log::trace!("open is none, disk is some -> true");
            return true;
        }

        // If we have a current file, but no on disk file, still return false, it may
        // be in the process of being rotated, or simply deleted with the current file still
        // being written to.
        if open.is_some() && disk.is_none() {
            log::trace!("open is some, disk is none -> false");
            return false;
        }

        // Now we can compare the metadata of the 2 files.
        let open = open.unwrap();
        let disk = disk.unwrap();

        if self.inode(&disk) != self.inode(&open) {
            log::trace!("on disk inode differs from open inode -> true");
            return true;
        }

        // If the file on disk is smaller than the open file, it has been rotated
        // or truncated.
        if disk.len() < self.offset {
            log::trace!("file on disk is smaller than open file -> true");
            return true;
        }

        return false;
    }

    /// Get the size of the file. This is taken directly from disk, so may not be the
    /// exact file currently being read by this reader.
    pub fn file_size(&self) -> u64 {
        if let Ok(metadata) = std::fs::metadata(&self.filename) {
            metadata.len()
        } else {
            0
        }
    }

    #[cfg(unix)]
    fn inode(&self, m: &std::fs::Metadata) -> Option<u64> {
        Some(m.ino())
    }

    #[cfg(not(unix))]
    fn inode(&self, _m: &std::fs::Metadata) -> Option<u64> {
        None
    }
}

#[derive(Debug)]
pub struct Metadata {
    pub filename: String,
    pub lineno: u64,
    pub size: u64,
    pub inode: Option<u64>,
}
