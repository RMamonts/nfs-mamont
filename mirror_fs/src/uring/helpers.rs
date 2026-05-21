use std::ffi::CString;
use std::io;

use crate::uring::types::StatxData;

pub fn statx_to_data(_path: &CString, statx: &libc::statx) -> io::Result<StatxData> {
    Ok(StatxData {
        mode: statx.stx_mode as u32,
        nlink: statx.stx_nlink,
        uid: statx.stx_uid,
        gid: statx.stx_gid,
        size: statx.stx_size,
        blocks: statx.stx_blocks,
        dev_major: statx.stx_dev_major,
        dev_minor: statx.stx_dev_minor,
        ino: statx.stx_ino,
        atime_sec: statx.stx_atime.tv_sec,
        atime_nsec: statx.stx_atime.tv_nsec as i64,
        mtime_sec: statx.stx_mtime.tv_sec,
        mtime_nsec: statx.stx_mtime.tv_nsec as i64,
        ctime_sec: statx.stx_ctime.tv_sec,
        ctime_nsec: statx.stx_ctime.tv_nsec as i64,
    })
}
