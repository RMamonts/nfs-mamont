//! Implements parsing for [`read::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::{u32, u64};
use crate::parser::Result;
use crate::vfs::read;

/// Parses the arguments for an NFSv3 `READ` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<read::Args> {
    Ok(read::Args { file: file::handle(src)?, offset: u64(src)?, count: u32(src)? })
}
