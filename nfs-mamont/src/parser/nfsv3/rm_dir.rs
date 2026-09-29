//! Implements parsing for [`rm_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::rm_dir;

/// Parses the arguments for an NFSv3 `RMDIR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<rm_dir::Args> {
    Ok(rm_dir::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
    })
}
