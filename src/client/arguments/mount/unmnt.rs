use std::io;
use std::io::Write;

use crate::mount::umnt::Args;
use crate::serializer::files::file_path;

/// Serializes the arguments [`UnmountArgs`] for a Mount `UNMOUNT` operation to the provided `Write` destination.
pub fn unmount_args(dest: &mut impl Write, arg: Args) -> io::Result<()> {
    file_path(dest, arg.dirpath)
}
