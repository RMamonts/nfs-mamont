use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::nfs::nfs_time;
use crate::serializer::{option, u32, u64};
use crate::vfs::fs_info;

#[allow(dead_code)]
pub fn fs_info_res_ok(dest: &mut impl Write, arg: fs_info::Success) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, attr))?;
    u32(dest, arg.read_max)?;
    u32(dest, arg.read_pref)?;
    u32(dest, arg.read_mult)?;
    u32(dest, arg.write_max)?;
    u32(dest, arg.write_pref)?;
    u32(dest, arg.write_mult)?;
    u32(dest, arg.read_dir_pref)?;
    u64(dest, arg.max_file_size)?;
    nfs_time(dest, arg.time_delta)?;
    u32(dest, arg.properties.0)
}

#[allow(dead_code)]
pub fn fs_info_res_fail(dest: &mut impl Write, arg: fs_info::Fail) -> io::Result<()> {
    option(dest, arg.root_attr, |attr, dest| file_attr(dest, attr))
}
