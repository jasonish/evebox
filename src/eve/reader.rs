// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Seek;
use std::io::SeekFrom;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use tracing::debug;
use tracing::error;
use tracing::trace;
use tracing::warn;

#[derive(thiserror::Error, Debug)]
pub(crate) enum EveReaderError {
    #[error("failed to parse event on line {line}")]
    ParseError {
        line: u64,
        #[source]
        source: serde_json::Error,
    },
    #[error("io error: {0}")]
    IoError(std::io::Error),
}

impl From<std::io::Error> for EveReaderError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

pub(crate) struct EveReader {
    pub filename: PathBuf,
    line: String,
    reader: Option<BufReader<std::fs::File>>,
    lineno: u64,
    offset: u64,
    unterminated: bool,
}

impl EveReader {
    pub(crate) fn new(filename: PathBuf) -> Self {
        Self {
            filename,
            line: String::new(),
            reader: None,
            lineno: 0,
            offset: 0,
            unterminated: false,
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

    pub fn goto_lineno(&mut self, lineno: u64) -> Result<u64, EveReaderError> {
        if self.reader.is_none() {
            self.open()?;
        }
        let mut count = 0;
        for _i in 0..lineno {
            if self.next_line(false)?.is_none() {
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
            let line = self.next_line(false)?;
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
        if let Some(reader) = &mut self.reader
            && let Ok(pos) = reader.stream_position()
        {
            return pos;
        }
        0
    }

    fn next_line(&mut self, accept_unterminated: bool) -> Result<Option<&str>, EveReaderError> {
        self.line.clear();
        self.unterminated = false;
        if let Some(reader) = &mut self.reader {
            let pos = reader.stream_position()?;
            let n = reader.read_line(&mut self.line)?;
            if n > 0 {
                if !self.line.ends_with('\n') {
                    if accept_unterminated {
                        self.offset = pos + n as u64;
                        self.lineno += 1;
                        self.unterminated = true;
                        return Ok(Some(self.line.trim()));
                    } else {
                        trace!(
                            "Line does not end with new line character, seeking back to {}",
                            pos
                        );
                        reader.seek(SeekFrom::Start(pos))?;
                    }
                } else {
                    self.offset = pos + n as u64;
                    self.lineno += 1;
                    let line = self.line.trim();
                    return Ok(Some(line));
                }
            }
        }
        Ok(None)
    }

    /// Not named next as we don't implement the iterator pattern (yet).
    pub fn next_record(&mut self) -> Result<Option<serde_json::Value>, EveReaderError> {
        self.next_record_inner(false)
    }

    /// Read a record from a static file, accepting a final line without a
    /// newline terminator.
    pub(crate) fn next_file_record(&mut self) -> Result<Option<serde_json::Value>, EveReaderError> {
        self.next_record_inner(true)
    }

    fn next_record_inner(
        &mut self,
        accept_unterminated: bool,
    ) -> Result<Option<serde_json::Value>, EveReaderError> {
        if self.reader.is_none() {
            self.open()?;
        }
        if self.reader.is_some() {
            loop {
                let line = match self.next_line(accept_unterminated)? {
                    None => break,
                    Some("") => continue,
                    Some(line) => line,
                };
                match serde_json::from_str(line) {
                    Ok(record) => return Ok(Some(record)),
                    Err(source) => {
                        if self.unterminated {
                            warn!(
                                "Ignoring unparseable, unterminated line {} (truncated file?)",
                                self.lineno
                            );
                            break;
                        }
                        return Err(EveReaderError::ParseError {
                            line: self.lineno,
                            source,
                        });
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn metadata(&self) -> Option<Metadata> {
        if let Some(reader) = &self.reader {
            match reader.get_ref().metadata() {
                Err(err) => {
                    error!("Failed to get metadata for open reader: {}", err);
                    return None;
                }
                Ok(meta) => {
                    let metadata = Metadata {
                        filename: self.filename.display().to_string(),
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
                    debug!("Failed to get metadata for open file: {}", err);
                    None
                }
                Ok(m) => Some(m),
            }
        } else {
            None
        };
        let disk: Option<std::fs::Metadata> = match std::fs::metadata(&self.filename) {
            Err(err) => {
                trace!("Failed to get metadata for file on disk: {}", err);
                None
            }
            Ok(m) => Some(m),
        };

        // If neither, then return false.
        if open.is_none() && disk.is_none() {
            trace!("open is none, disk is none -> false");
            return false;
        }

        // If we don't have an open file, but there is an on disk file, return true.
        if open.is_none() && disk.is_some() {
            trace!("open is none, disk is some -> true");
            return true;
        }

        // If we have a current file, but no on disk file, still return false, it may
        // be in the process of being rotated, or simply deleted with the current file still
        // being written to.
        if open.is_some() && disk.is_none() {
            trace!("open is some, disk is none -> false");
            return false;
        }

        // Now we can compare the metadata of the 2 files.
        let open = open.unwrap();
        let disk = disk.unwrap();

        if self.inode(&disk) != self.inode(&open) {
            trace!("on disk inode differs from open inode -> true");
            return true;
        }

        // If the file on disk is smaller than the open file, it has been rotated
        // or truncated.
        if disk.len() < self.offset {
            trace!("file on disk is smaller than open file -> true");
            return true;
        }

        false
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

/// EVE record reader over a non-seekable stream such as stdin.
///
/// Follows the same rules as [`EveReader::next_file_record`]: blank lines
/// are skipped, an unterminated final line is accepted, and an
/// unparseable, unterminated final line is ignored as truncated input.
pub(crate) struct EveStreamReader<R: BufRead> {
    reader: R,
    line: String,
    lineno: u64,
}

impl<R: BufRead> EveStreamReader<R> {
    pub(crate) fn new(reader: R) -> Self {
        Self {
            reader,
            line: String::new(),
            lineno: 0,
        }
    }

    pub(crate) fn next_record(&mut self) -> Result<Option<serde_json::Value>, EveReaderError> {
        loop {
            self.line.clear();
            let n = self.reader.read_line(&mut self.line)?;
            if n == 0 {
                return Ok(None);
            }
            let terminated = self.line.ends_with('\n');
            self.lineno += 1;
            let line = self.line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str(line) {
                Ok(record) => return Ok(Some(record)),
                Err(source) => {
                    if !terminated {
                        warn!(
                            "Ignoring unparseable, unterminated line {} (truncated input?)",
                            self.lineno
                        );
                        return Ok(None);
                    }
                    return Err(EveReaderError::ParseError {
                        line: self.lineno,
                        source,
                    });
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Metadata {
    pub filename: String,
    pub lineno: u64,
    pub size: u64,
    pub inode: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const RECORD: &str = r#"{"timestamp":"2026-07-15T12:00:00.000000+0000","event_type":"stats"}"#;

    fn reader_for(contents: &[u8]) -> (tempfile::TempDir, EveReader) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eve.json");
        let mut file = File::create(&path).unwrap();
        file.write_all(contents).unwrap();
        (dir, EveReader::new(path))
    }

    #[test]
    fn blank_lines_are_skipped() {
        let contents = format!("{RECORD}\n\n{RECORD}\n");
        let (_dir, mut reader) = reader_for(contents.as_bytes());
        assert!(reader.next_file_record().unwrap().is_some());
        assert!(reader.next_file_record().unwrap().is_some());
        assert!(reader.next_file_record().unwrap().is_none());
    }

    #[test]
    fn unterminated_final_record_is_accepted() {
        let contents = format!("{RECORD}\n{RECORD}");
        let (_dir, mut reader) = reader_for(contents.as_bytes());
        assert!(reader.next_file_record().unwrap().is_some());
        assert!(reader.next_file_record().unwrap().is_some());
        assert!(reader.next_file_record().unwrap().is_none());
    }

    #[test]
    fn truncated_final_line_is_ignored() {
        let contents = format!("{RECORD}\n{}", &RECORD[..RECORD.len() - 10]);
        let (_dir, mut reader) = reader_for(contents.as_bytes());
        assert!(reader.next_file_record().unwrap().is_some());
        assert!(reader.next_file_record().unwrap().is_none());
    }

    #[test]
    fn malformed_terminated_line_is_an_error() {
        let contents = format!("{RECORD}\n{{not-json}}\n{RECORD}\n");
        let (_dir, mut reader) = reader_for(contents.as_bytes());
        assert!(reader.next_file_record().unwrap().is_some());
        let err = reader.next_file_record().unwrap_err();
        assert!(matches!(err, EveReaderError::ParseError { line: 2, .. }));
    }

    #[test]
    fn stream_reader_accepts_unterminated_final_record() {
        let contents = format!("{RECORD}\n\n{RECORD}");
        let mut reader = EveStreamReader::new(std::io::Cursor::new(contents));
        assert!(reader.next_record().unwrap().is_some());
        assert!(reader.next_record().unwrap().is_some());
        assert!(reader.next_record().unwrap().is_none());
    }

    #[test]
    fn stream_reader_ignores_truncated_final_line() {
        let contents = format!("{RECORD}\n{}", &RECORD[..RECORD.len() - 10]);
        let mut reader = EveStreamReader::new(std::io::Cursor::new(contents));
        assert!(reader.next_record().unwrap().is_some());
        assert!(reader.next_record().unwrap().is_none());
    }

    #[test]
    fn stream_reader_errors_on_malformed_terminated_line() {
        let contents = format!("{RECORD}\n{{not-json}}\n");
        let mut reader = EveStreamReader::new(std::io::Cursor::new(contents));
        assert!(reader.next_record().unwrap().is_some());
        let err = reader.next_record().unwrap_err();
        assert!(matches!(err, EveReaderError::ParseError { line: 2, .. }));
    }

    #[test]
    fn tailing_reader_waits_for_unterminated_lines() {
        let contents = format!("{RECORD}\n{RECORD}");
        let (_dir, mut reader) = reader_for(contents.as_bytes());
        assert!(reader.next_record().unwrap().is_some());
        // The tailing reader must not consume the unterminated line, it
        // may still be written to.
        assert!(reader.next_record().unwrap().is_none());
        assert!(reader.next_record().unwrap().is_none());
    }
}
