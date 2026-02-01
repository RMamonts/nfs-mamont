use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::{option, u32};
use crate::vfs::access;

#[allow(dead_code)]
pub fn access_res_ok(dest: &mut impl Write, arg: access::Success) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, attr))?;
    u32(dest, arg.access.0)
}

#[allow(dead_code)]
pub fn access_res_fail(dest: &mut impl Write, arg: access::Fail) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, attr))
}
