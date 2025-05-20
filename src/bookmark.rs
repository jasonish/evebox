// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::eve;
use serde::{Deserialize, Serialize};
use std::io::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;
use tracing::trace;
use tracing::warn;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Bookmark {
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub sys: BookmarkSys,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct BookmarkSys {
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

    pub fn from_file(filename: &Path) -> Result<Bookmark, Box<dyn std::error::Error>> {
        let mut file = std::fs::File::open(filename)?;
        let mut body = String::new();
        file.read_to_string(&mut body)?;
        let bookmark: Bookmark = serde_json::from_str(&body)?;
        Ok(bookmark)
    }

    pub fn write(&self, filename: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        trace!("Writing bookmark {}", filename.to_str().unwrap());
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
        true
    }

    #[cfg(not(unix))]
    fn check_inode(&self, _meta: &std::fs::Metadata) -> bool {
        true
    }
}

pub(crate) fn bookmark_filename<P: AsRef<Path>>(input_filename: P, bookmark_dir: &str) -> PathBuf {
    let directory = match std::fs::canonicalize(bookmark_dir) {
        Ok(directory) => directory,
        Err(err) => {
            warn!("Failed to canonicalize directory {}: {}", bookmark_dir, err);
            std::path::PathBuf::from(bookmark_dir)
        }
    };

    let hash = md5::compute(input_filename.as_ref().display().to_string());
    let filename = format!("{hash:x}.bookmark");

    directory.join(filename)
}
