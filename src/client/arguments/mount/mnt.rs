use std::io;
use std::io::Write;

use crate::mount::mnt;
use crate::serializer::files::file_path;

/// Serializes the arguments [`mnt::Args`] for a Mount `MOUNT` operation to the provided `Write` destination.
pub fn mount_args(dest: &mut impl Write, arg: mnt::Args) -> io::Result<()> {
    file_path(dest, arg.dirpath)
}
