// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub(crate) fn file_size(filename: &str) -> anyhow::Result<u64> {
    let meta = std::fs::metadata(filename)?;
    Ok(meta.len())
}
