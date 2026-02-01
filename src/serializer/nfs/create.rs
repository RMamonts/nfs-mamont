use std::io;
use std::io::Write;

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::{file_attr, wcc_data};
use crate::serializer::option;
use crate::vfs::create;

#[allow(dead_code)]
pub fn create_res_ok(dest: &mut impl Write, arg: create::Success) -> io::Result<()> {
    option(dest, arg.file, |fh, dest| file_handle(dest, fh))?;
    option(dest, arg.attr, |attr, dest| file_attr(dest, attr))?;
    wcc_data(dest, arg.wcc_data)
}

#[allow(dead_code)]
pub fn create_res_fail(dest: &mut impl Write, arg: create::Fail) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}
