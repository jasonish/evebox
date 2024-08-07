// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::path::Path;

pub(crate) fn file_size(filename: &Path) -> anyhow::Result<u64> {
    let meta = std::fs::metadata(filename)?;
    Ok(meta.len())
}

pub(crate) fn test_directory_is_writable<P: AsRef<Path>>(directory: P) -> bool {
    tempfile::tempfile_in(directory).is_ok()
}
