//! XDR serializers for the NFSv3 `ACCESS` procedure.

use std::io;
use std::io::Write;

use crate::serializer::files::file_attr;
use crate::serializer::{option, u32};
use crate::vfs::access;

/// Serializes [`access::Success`] (ACCESS3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: access::Success) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, &attr))?;
    u32(dest, arg.access.0)
}

/// Serializes [`access::Fail`] (ACCESS3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: access::Fail) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, &attr))
}
