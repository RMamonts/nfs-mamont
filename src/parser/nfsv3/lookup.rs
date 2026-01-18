//! Implements parsing for [`lookup::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::lookup;

pub fn args(src: &mut impl Read) -> Result<lookup::Args> {
    Ok(lookup::Args { parent: file::handle(src)?, name: string(src)? })
}
