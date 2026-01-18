//! Implements parsing for [`fs_stat::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::fs_stat;

pub fn args(src: &mut impl Read) -> Result<fs_stat::Args> {
    Ok(fs_stat::Args { root: file::handle(src)? })
}
