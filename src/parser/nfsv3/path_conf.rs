//! Implements parsing for [`path_conf::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::path_conf;

pub fn args(src: &mut impl Read) -> Result<path_conf::Args> {
    Ok(path_conf::Args { file: file::handle(src)? })
}
