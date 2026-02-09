use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::{option, u32, u64};
use crate::vfs::fs_stat;

#[allow(dead_code)]
pub fn result_ok(dest: &mut impl Write, arg: fs_stat::Success) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, attr))?;
    u64(dest, arg.total_bytes)?;
    u64(dest, arg.free_bytes)?;
    u64(dest, arg.available_bytes)?;
    u64(dest, arg.total_files)?;
    u64(dest, arg.free_files)?;
    u64(dest, arg.available_files)?;
    u32(dest, arg.invarsec)
}

pub fn result_fail(dest: &mut impl Write, arg: fs_stat::Fail) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, attr))
}
