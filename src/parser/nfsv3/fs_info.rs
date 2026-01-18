//! Implements parsing for [`fs_info::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::fs_info;

pub fn args(src: &mut impl Read) -> Result<fs_info::Args> {
    Ok(fs_info::Args { root: file::handle(src)? })
}
