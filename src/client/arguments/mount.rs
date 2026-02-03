use crate::parser::mount::{MountArgs, UnmountArgs};
use crate::serializer::nfs::files::file_path;
use std::io;
use std::io::Write;

pub fn mount_args(dest: &mut impl Write, arg: MountArgs) -> io::Result<()> {
    file_path(dest, arg.0)
}

pub fn unmount_args(dest: &mut impl Write, arg: UnmountArgs) -> io::Result<()> {
    file_path(dest, arg.0)
}
