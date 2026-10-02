//! Implements parsing for [`write::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::{u32, u64, variant};
use crate::parser::Result;
use crate::vfs::write;
use crate::vfs::write::StableHow;

/// Parses the arguments for an NFSv3 `WRITE` operation from the provided `Read` source.
/// Function returns either `parser::Error` or `write::ArgPartial`.
/// Later one is not a complete structure of NFSv3 `WRITE` procedure.
/// Opaque data must be parsed separately
pub fn args(src: &mut impl Read) -> Result<write::ArgsPartial> {
    Ok(write::ArgsPartial {
        file: file::handle(src)?,
        offset: u64(src)?,
        size: u32(src)?,
        stable: variant::<StableHow>(src)?,
    })
}
