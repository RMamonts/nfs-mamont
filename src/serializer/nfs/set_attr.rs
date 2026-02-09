//! XDR serializers for the NFSv3 `SETATTR` procedure.

use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::vfs::set_attr;

/// Serializes [`set_attr::Success`] (SETATTR3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: set_attr::Success) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}

/// Serializes [`set_attr::Fail`] (SETATTR3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: set_attr::Fail) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}
