//! Implements parsing for [`remove::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::remove;

/// Parses the arguments for an NFSv3 `REMOVE` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<remove::Args> {
    Ok(remove::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
    })
}
