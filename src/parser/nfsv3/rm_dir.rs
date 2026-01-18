//! Implements parsing for [`rm_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::rm_dir;

pub fn args(src: &mut impl Read) -> Result<rm_dir::Args> {
    Ok(rm_dir::Args { dir: file::handle(src)?, name: string(src)? })
}
