//! Implements parsing for [`create::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::create;

pub fn how(_src: &mut impl Read) -> Result<create::How> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<create::Args> {
    Ok(create::Args { dir: file::handle(src)?, name: string(src)?, how: how(src)? })
}
