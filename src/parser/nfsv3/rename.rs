//! Implements parsing for [`rename::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::rename;

pub fn args(src: &mut impl Read) -> Result<rename::Args> {
    Ok(rename::Args {
        from_dir: file::handle(src)?,
        from_name: string(src)?,
        to_dir: file::handle(src)?,
        to_name: string(src)?,
    })
}
