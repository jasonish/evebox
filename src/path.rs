// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::debug;

const SYSTEM_DATA_DIR: &str = "/var/lib/evebox";

pub(crate) fn expand(path: &str) -> anyhow::Result<Vec<std::path::PathBuf>> {
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

pub(crate) fn data_directory() -> Option<PathBuf> {
    let system_default = Path::new(SYSTEM_DATA_DIR);
    if system_default.exists() {
        debug!(
            "Found default system data directory {}",
            system_default.display()
        );
        let test_filename = system_default.join(format!("{}.test_write", uuid::Uuid::new_v4()));
        if std::fs::File::create(&test_filename).is_ok() {
            let _ = std::fs::remove_file(&test_filename);
            return Some(system_default.to_owned());
        } else {
            debug!(
                "Default system data directory {} not writable",
                system_default.display()
            );
        }
    }

    if let Some(dirs) = directories::ProjectDirs::from("org", "evebox", "evebox") {
        return Some(dirs.config_local_dir().to_owned());
    }
    None
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
