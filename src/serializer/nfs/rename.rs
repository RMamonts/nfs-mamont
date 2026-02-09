//! XDR serializers for the NFSv3 `RENAME` procedure.

use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::vfs::rename;

/// Serializes [`rename::Success`] (RENAME3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: rename::Success) -> io::Result<()> {
    wcc_data(dest, arg.from_dir_wcc)?;
    wcc_data(dest, arg.to_dir_wcc)
}

/// Serializes [`rename::Fail`] (RENAME3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: rename::Fail) -> io::Result<()> {
    wcc_data(dest, arg.from_dir_wcc)?;
    wcc_data(dest, arg.to_dir_wcc)
}
