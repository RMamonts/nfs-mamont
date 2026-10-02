//! Implements parsing for [`access::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u32;
use crate::parser::Result;
use crate::vfs::access;

/// Parses the arguments for an NFSv3 `ACCESS` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<access::Args> {
    Ok(access::Args { file: file::handle(src)?, mask: access::Mask::from_wire(u32(src)?) })
}
