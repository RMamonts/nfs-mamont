//! Implements parsing for [`read::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u64;
use crate::parser::Result;
use crate::vfs::read;

pub fn args(src: &mut impl Read) -> Result<read::Args> {
    Ok(read::Args { file: file::handle(src)?, offset: u64(src)?, count: u64(src)? })
}
