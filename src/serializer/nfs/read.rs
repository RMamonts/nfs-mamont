use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::{bool, option, u64};
use crate::vfs::read;

// slice is read separately
pub fn result_ok_part(dest: &mut impl Write, arg: read::SuccessPartial) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))?;
    u64(dest, arg.count)?;
    bool(dest, arg.eof)
}

pub fn result_fail(dest: &mut impl Write, arg: read::Fail) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))
}
