use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::vfs::remove;

#[allow(dead_code)]
pub fn remove_res_ok(dest: &mut impl Write, arg: remove::Success) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}

#[allow(dead_code)]
pub fn remove_res_fail(dest: &mut impl Write, arg: remove::Fail) -> io::Result<()> {
    wcc_data(dest, arg.dir_wcc)
}
