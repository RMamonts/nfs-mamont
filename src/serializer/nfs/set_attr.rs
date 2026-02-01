use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::vfs::set_attr;

#[allow(dead_code)]
pub fn set_attr_res_ok(dest: &mut impl Write, arg: set_attr::Success) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}

#[allow(dead_code)]
pub fn set_attr_res_fail(dest: &mut impl Write, arg: set_attr::Fail) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}
