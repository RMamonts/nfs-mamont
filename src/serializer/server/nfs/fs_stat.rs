//! XDR serializers for the NFSv3 `FSSTAT` procedure.

use std::io;
use std::io::Write;

use crate::serializer::files::file_attr;
use crate::serializer::{option, u32, u64};
use crate::vfs::fs_stat;

/// Serializes [`fs_stat::Success`] (FSSTAT3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: fs_stat::Success) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, &attr))?;
    u64(dest, arg.total_bytes)?;
    u64(dest, arg.free_bytes)?;
    u64(dest, arg.available_bytes)?;
    u64(dest, arg.total_files)?;
    u64(dest, arg.free_files)?;
    u64(dest, arg.available_files)?;
    u32(dest, arg.invarsec)
}

/// Serializes [`fs_stat::Fail`] (FSSTAT3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: fs_stat::Fail) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, &attr))
}
