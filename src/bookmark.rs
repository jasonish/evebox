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

use crate::eve;
use crate::logger::log;
use serde::{Deserialize, Serialize};
use std::io::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Bookmark {
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub sys: BookmarkSys,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BookmarkSys {
    pub inode: Option<u64>,
}

impl Bookmark {
    pub fn from_metadata(meta: &eve::reader::Metadata) -> Bookmark {
        Bookmark {
            path: meta.filename.clone(),
            offset: meta.lineno,
            size: meta.size,
            sys: BookmarkSys { inode: meta.inode },
        }
    }

    pub fn from_file(filename: &PathBuf) -> Result<Bookmark, Box<dyn std::error::Error>> {
        let mut file = std::fs::File::open(filename)?;
        let mut body = String::new();
        file.read_to_string(&mut body)?;
        let bookmark: Bookmark = serde_json::from_str(&body)?;
        Ok(bookmark)
    }

    pub fn write(
        &self,
        filename: &PathBuf,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::trace!("Writing bookmark {}", filename.to_str().unwrap());
        let mut file = std::fs::File::create(filename)?;
        file.write_all(serde_json::to_string(self).unwrap().as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }

    pub fn is_valid(&self) -> Result<(), Box<dyn std::error::Error>> {
        let m = std::fs::metadata(&self.path)?;
        if !self.check_inode(&m) {
            return Err("inode mismatch".into());
        }
        if m.len() < self.size {
            return Err("current file size less than bookmark".into());
        }
        Ok(())
    }

    #[cfg(unix)]
    fn check_inode(&self, meta: &std::fs::Metadata) -> bool {
        if let Some(inode) = self.sys.inode {
            if inode != meta.ino() {
                return false;
            }
        }
        return true;
    }

    #[cfg(not(unix))]
    fn check_inode(&self, _meta: &std::fs::Metadata) -> bool {
        return true;
    }
}

pub fn bookmark_filename(input_filename: &str, bookmark_dir: &str) -> std::path::PathBuf {
    let directory = match std::fs::canonicalize(bookmark_dir) {
        Ok(directory) => directory,
        Err(err) => {
            log::warn!("Failed to canonicalize directory {}: {}", bookmark_dir, err);
            std::path::PathBuf::from(bookmark_dir)
        }
    };

    let hash = md5::compute(&input_filename);
    let filename = format!("{:x}.bookmark", hash);
    let path = directory.join(filename);
    return path;
}
