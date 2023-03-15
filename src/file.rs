// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

pub(crate) fn file_size(filename: &str) -> anyhow::Result<u64> {
    let meta = std::fs::metadata(filename)?;
    Ok(meta.len())
}

#[derive(Debug)]
pub struct FsUsage {
    pub bytes_free: u64,
    pub bytes_used: u64,
    pub bytes_total: u64,
    pub percent_used: u64,
    pub percent_free: u64,
}

#[cfg(target_family = "unix")]
pub mod unix {
    use super::FsUsage;
    use libc::statvfs;

    pub fn _fs_usage(filename: &str) -> anyhow::Result<FsUsage> {
        let filename = std::ffi::CString::new(filename)?;
        unsafe {
            let mut sfs: statvfs = std::mem::zeroed();
            if statvfs(filename.as_ptr(), &mut sfs) != 0 {
                anyhow::bail!("statfs failed");
            }
            #[allow(clippy::unnecessary_cast)]
            let block_size = sfs.f_bsize as u64;
            #[allow(clippy::unnecessary_cast)]
            let bytes_total = sfs.f_blocks as u64 * block_size;
            #[allow(clippy::unnecessary_cast)]
            let bytes_free = sfs.f_bavail as u64 * block_size;
            let bytes_used = bytes_total - bytes_free;
            let percent_used = bytes_used * 100 / bytes_total;
            let percent_free = 100 - percent_used;
            Ok(FsUsage {
                bytes_free,
                bytes_used,
                bytes_total,
                percent_used,
                percent_free,
            })
        }
    }
}

#[cfg(target_family = "unix")]
pub fn _has_fs_usage() -> bool {
    true
}

#[cfg(not(target_family = "unix"))]
pub fn _has_fs_usage() -> bool {
    false
}

#[cfg(target_family = "unix")]
pub fn _fs_usage(filename: &str) -> anyhow::Result<FsUsage> {
    unix::_fs_usage(filename)
}

#[cfg(not(target_family = "unix"))]
pub fn _fs_usage(_filename: &str) -> anyhow::Result<FsUsage> {
    unimplemented!()
}
