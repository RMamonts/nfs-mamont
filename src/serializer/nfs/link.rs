use std::io;
use std::io::Write;

use crate::serializer::nfs::files::{file_attr, wcc_data};
use crate::serializer::option;
use crate::vfs::link;

#[allow(dead_code)]
pub fn link_res_ok(dest: &mut impl Write, arg: link::Success) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))?;
    wcc_data(dest, arg.dir_wcc)
}

#[allow(dead_code)]
pub fn link_res_fail(dest: &mut impl Write, arg: link::Fail) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))?;
    wcc_data(dest, arg.dir_wcc)
}
