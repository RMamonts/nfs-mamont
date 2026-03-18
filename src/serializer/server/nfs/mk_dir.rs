//! XDR serializers for the NFSv3 `MKDIR` procedure.

use std::io;
use std::io::Write;

use crate::serializer::files::{file_attr, file_handle, wcc_data};
use crate::serializer::option;
use crate::vfs::mk_dir;

/// Serializes [`mk_dir::Success`] (MKDIR3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: mk_dir::Success) -> io::Result<()> {
    option(dest, arg.file, |fh, dest| file_handle(dest, fh))?;
    option(dest, arg.attr, |attr, dest| file_attr(dest, &attr))?;
    wcc_data(dest, arg.wcc_data)
}

/// Serializes [`mk_dir::Fail`] (MKDIR3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: mk_dir::Fail) -> io::Result<()> {
    wcc_data(dest, arg.dir_wcc)
}
