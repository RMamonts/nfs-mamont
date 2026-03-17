use std::io;
use std::io::Write;

use crate::mount::umnt;
use crate::serializer::files::file_path;

/// Serializes the arguments [`umnt::Args`] for a Mount `UNMOUNT` operation to the provided `Write` destination.
pub fn unmount_args(dest: &mut impl Write, arg: umnt::Args) -> io::Result<()> {
    file_path(dest, arg.dirpath)
}
