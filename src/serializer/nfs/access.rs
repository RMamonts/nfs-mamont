//! XDR serializers for the NFSv3 `ACCESS` procedure.

use std::io;
use std::io::Write;

use crate::interface::vfs::access;
use crate::serializer::files::file_attr;
use crate::serializer::{option, u32};

/// Serializes [`access::Success`] (ACCESS3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: access::Success) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, &attr))?;
    u32(dest, arg.access.bits())
}

/// Serializes [`access::Fail`] (ACCESS3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: access::Fail) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, &attr))
}
