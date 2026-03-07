use std::io::{self, Write};

use crate::mount::{mnt::MountArgs, umnt::UnmountArgs};
use crate::serializer::files::file_path;

pub fn mount_args(dest: &mut impl Write, arg: MountArgs) -> io::Result<()> {
    file_path(dest, arg.0)
}

pub fn unmount_args(dest: &mut impl Write, arg: UnmountArgs) -> io::Result<()> {
    file_path(dest, arg.0)
}
