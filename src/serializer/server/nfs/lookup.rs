//! XDR serializers for the NFSv3 `LOOKUP` procedure.

use std::io;
use std::io::{ErrorKind, Write};

use crate::serializer::files::{file_attr, file_handle};
use crate::serializer::option;
use crate::vfs::lookup;

/// Serializes [`lookup::Success`] (LOOKUP3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: lookup::Success) -> io::Result<()> {
    let file = arg.file.ok_or(io::Error::new(ErrorKind::NotFound, "no such file"))?;
    file_handle(dest, file)?;
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, &attr))?;
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, &attr))
}

/// Serializes [`lookup::Fail`] (LOOKUP3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: lookup::Fail) -> io::Result<()> {
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, &attr))
}
