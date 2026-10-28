//! Implements parsing for [`symlink::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::{file_name, file_path};
use crate::parser::nfsv3::set_attr::new_attr;
use crate::parser::Result;
use crate::vfs::symlink;

/// Parses the arguments for an NFSv3 `SYMLINK` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<symlink::Args> {
    Ok(symlink::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        attr: new_attr(src)?,
        path: file_path(src)?,
    })
}
