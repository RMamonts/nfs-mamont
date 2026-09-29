//! Implements parsing for [`lookup::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::lookup;

/// Parses the arguments for an NFSv3 `LOOKUP` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<lookup::Args> {
    Ok(lookup::Args { parent: file::handle(src)?, name: file_name(src)? })
}
