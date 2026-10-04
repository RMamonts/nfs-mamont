//! Implements parsing for [`read_link::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::read_link;

/// Parses the arguments for an NFSv3 `READLINK` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<read_link::Args> {
    Ok(read_link::Args { file: file::handle(src)? })
}
