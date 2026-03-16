//! XDR serializers for the NFSv3 `REMOVE` procedure.

use std::io;
use std::io::Write;

use crate::interface::vfs::remove;
use crate::serializer::files::wcc_data;

/// Serializes [`remove::Success`] (REMOVE3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: remove::Success) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}

/// Serializes [`remove::Fail`] (REMOVE3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: remove::Fail) -> io::Result<()> {
    wcc_data(dest, arg.dir_wcc)
}
