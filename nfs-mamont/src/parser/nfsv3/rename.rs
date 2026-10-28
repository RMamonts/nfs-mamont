//! Implements parsing for [`rename::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::rename;

/// Parses the arguments for an NFSv3 `RENAME` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<rename::Args> {
    Ok(rename::Args {
        from: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        to: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
    })
}
