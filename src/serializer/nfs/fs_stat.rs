use crate::serializer::nfs::files::file_attr;
use crate::serializer::{option, u32};
use crate::vfs::fs_stat;
use std::io;
use std::io::Write;

#[allow(dead_code)]
pub fn fs_stat_res_ok<S: Write>(dest: &mut S, arg: fs_stat::Success) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, attr))?;
    u32(dest, arg.total_bytes)?;
    u32(dest, arg.free_bytes)?;
    u32(dest, arg.available_bytes)?;
    u32(dest, arg.total_files)?;
    u32(dest, arg.free_files)?;
    u32(dest, arg.available_files)?;
    u32(dest, arg.invarsec)
}

#[allow(dead_code)]
pub fn fs_stat_res_fail(dest: &mut impl Write, arg: fs_stat::Fail) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, attr))
}
