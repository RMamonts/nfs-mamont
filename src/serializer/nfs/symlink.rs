use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::{file_attr, wcc_data};
use crate::serializer::option;
use crate::vfs::symlink;
use std::io;
use std::io::Write;

pub fn result_ok(dest: &mut impl Write, arg: symlink::Success) -> io::Result<()> {
    option(dest, arg.file, |fh, dest| file_handle(dest, fh))?;
    option(dest, arg.attr, |attr, dest| file_attr(dest, attr))?;
    wcc_data(dest, arg.wcc_data)
}

pub fn result_fail(dest: &mut impl Write, arg: symlink::Fail) -> io::Result<()> {
    wcc_data(dest, arg.dir_wcc)
}
