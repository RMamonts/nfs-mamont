use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::vfs::rm_dir;

#[allow(dead_code)]
pub fn rmdir_res_ok(dest: &mut impl Write, arg: rm_dir::Success) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}

#[allow(dead_code)]
pub fn rmdir_res_fail(dest: &mut impl Write, arg: rm_dir::Fail) -> io::Result<()> {
    wcc_data(dest, arg.dir_wcc)
}
