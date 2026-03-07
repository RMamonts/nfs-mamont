use std::io;
use std::io::Write;

use crate::mount::mnt::MountArgs;
use crate::serializer::files::file_path;

/// Serializes the arguments [`MountArgs`] for a Mount `MOUNT` operation to the provided `Write` destination.
pub fn mount_args(dest: &mut impl Write, arg: MountArgs) -> io::Result<()> {
    file_path(dest, arg.0)
}
