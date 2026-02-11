//! XDR serializers for the NFSv3 `GETATTR` procedure.

use crate::serializer::nfs::files::file_attr;
use crate::vfs::get_attr;
use std::io;
use std::io::Write;

/// Serializes [`get_attr::Success`] (GETATTR3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: get_attr::Success) -> io::Result<()> {
    file_attr(dest, &arg.object)
}

/// Serializes [`get_attr::Fail`] (GETATTR3resfail body) into XDR.
pub fn result_fail(_dest: &mut impl Write, _arg: get_attr::Fail) -> io::Result<()> {
    Ok(())
}
