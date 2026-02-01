use std::io;
use std::io::Write;

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_attr;
use crate::serializer::option;
use crate::vfs::lookup;

#[allow(dead_code)]
pub fn lookup_res_ok(dest: &mut impl Write, arg: lookup::Success) -> io::Result<()> {
    file_handle(dest, arg.file)?;
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))?;
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, attr))
}

#[allow(dead_code)]
pub fn lookup_res_fail(dest: &mut impl Write, arg: lookup::Fail) -> io::Result<()> {
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, attr))
}
