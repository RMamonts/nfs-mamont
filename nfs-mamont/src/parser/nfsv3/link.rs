//! Implements parsing for [`link::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::link;

/// Parses the arguments for an NFSv3 `LINK` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<link::Args> {
    Ok(link::Args {
        file: file::handle(src)?,
        link: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
    })
}
