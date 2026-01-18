//! Implements parsing for [`remove::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::remove;

pub fn args(src: &mut impl Read) -> Result<remove::Args> {
    Ok(remove::Args { dir: file::handle(src)?, name: string(src)? })
}
