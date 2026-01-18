//! Implements parsing for [`access::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u32;
use crate::parser::Result;
use crate::vfs::access;

pub fn args(src: &mut impl Read) -> Result<access::Args> {
    Ok(access::Args { file: file::handle(src)?, mask: access::Mask(u32(src)?) })
}
