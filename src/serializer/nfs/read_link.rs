use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::nfs::files::file_path;
use crate::serializer::option;
use crate::vfs::read_link;

pub fn result_ok(dest: &mut impl Write, arg: read_link::Success) -> io::Result<()> {
    option(dest, arg.symlink_attr, |attr, dest| file_attr(dest, attr))?;
    file_path(dest, arg.data)
}

pub fn result_fail(dest: &mut impl Write, arg: read_link::Fail) -> io::Result<()> {
    option(dest, arg.symlink_attr, |attr, dest| file_attr(dest, attr))
}
