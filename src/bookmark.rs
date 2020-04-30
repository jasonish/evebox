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
