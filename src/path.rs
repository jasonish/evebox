// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use anyhow::Result;
use std::path::Path;
use tracing::debug;

pub fn expand(path: &str) -> anyhow::Result<Vec<std::path::PathBuf>> {
    Ok(glob::glob(path)?.flatten().collect())
}

/// Check if a path exists, creating it if it doesn't.
pub(crate) fn ensure_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        debug!("Creating directory {}", path.display());
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::expand;

    #[test]
    fn test_expand() {
        let paths = expand("src/*.rs").unwrap();
        assert!(!paths.is_empty());
    }
}
