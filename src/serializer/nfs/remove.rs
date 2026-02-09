use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::vfs::remove;

pub fn result_ok(dest: &mut impl Write, arg: remove::Success) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}

#[allow(dead_code)]
pub fn result_fail(dest: &mut impl Write, arg: remove::Fail) -> io::Result<()> {
    wcc_data(dest, arg.dir_wcc)
}
