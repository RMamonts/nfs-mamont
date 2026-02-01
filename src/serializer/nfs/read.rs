use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::{bool, option, u64};
use crate::vfs::read;

// slice is read separately
#[allow(dead_code)]
pub fn read_res_ok_partial(dest: &mut impl Write, arg: read::SuccessPartial) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))?;
    u64(dest, arg.count)?;
    bool(dest, arg.eof)
}

#[allow(dead_code)]
pub fn read_res_fail(dest: &mut impl Write, arg: read::Fail) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))
}
