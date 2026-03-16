//! XDR serializers for the NFSv3 `COMMIT` procedure.

use std::io;
use std::io::Write;

use crate::serializer::array;
use crate::serializer::files::wcc_data;
use crate::vfs::commit;

/// Serializes [`commit::Success`] (COMMIT3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: commit::Success) -> io::Result<()> {
    wcc_data(dest, arg.file_wcc)?;
    array(dest, arg.verifier.0)
}

/// Serializes [`commit::Fail`] (COMMIT3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: commit::Fail) -> io::Result<()> {
    wcc_data(dest, arg.file_wcc)
}
