//! XDR serializers for the NFSv3 `RMDIR` procedure.

use std::io;
use std::io::Write;

use crate::interface::vfs::rm_dir;
use crate::serializer::files::wcc_data;

/// Serializes [`rm_dir::Success`] (RMDIR3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: rm_dir::Success) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}

/// Serializes [`rm_dir::Fail`] (RMDIR3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: rm_dir::Fail) -> io::Result<()> {
    wcc_data(dest, arg.dir_wcc)
}
