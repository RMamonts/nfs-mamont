//! XDR serializers for the NFSv3 `READ` procedure.

use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::{bool, option, u32};
use crate::vfs::read;

/// Serializes the non-payload part of [`read::SuccessPartial`] (READ3resok body) into XDR.
///
/// The actual read data bytes are sent separately.
pub fn result_ok_part(dest: &mut impl Write, arg: read::SuccessPartial) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, &attr))?;
    u32(dest, arg.count)?;
    bool(dest, arg.eof)
}

/// Serializes [`read::Fail`] (READ3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: read::Fail) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, &attr))
}
