use std::io::{self, Write};

use crate::mount::{mnt::MountArgs, umnt::UnmountArgs};
use crate::serializer::files::file_path;

/// Serializes the arguments [`MountArgs`] for a Mount `MOUNT` operation to the provided `Write` destination.
pub fn mount_args(dest: &mut impl Write, arg: MountArgs) -> io::Result<()> {
    file_path(dest, arg.0)
}

/// Serializes the arguments [`UnmountArgs`] for a Mount `UNMOUNT` operation to the provided `Write` destination.
pub fn unmount_args(dest: &mut impl Write, arg: UnmountArgs) -> io::Result<()> {
    file_path(dest, arg.0)
}
