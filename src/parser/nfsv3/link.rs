//! Implements parsing for [`link::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::link;

pub fn args(src: &mut impl Read) -> Result<link::Args> {
    Ok(link::Args { file: file::handle(src)?, dir: file::handle(src)?, name: string(src)? })
}
