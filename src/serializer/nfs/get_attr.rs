//! XDR serializers for the NFSv3 `GETATTR` procedure.

use std::io;
use std::io::Write;

use crate::serializer::nfs::file_handle;
use crate::vfs::get_attr;

/// Serializes [`get_attr::Success`] (GETATTR3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: get_attr::Success) -> io::Result<()> {
    file_handle(dest, arg.object)
}
