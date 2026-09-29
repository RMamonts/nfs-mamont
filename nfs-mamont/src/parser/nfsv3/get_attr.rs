//! Implements parsing for [`get_attr::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::get_attr;

/// Parses the arguments for an NFSv3 `GETATTR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<get_attr::Args> {
    Ok(get_attr::Args { file: file::handle(src)? })
}
