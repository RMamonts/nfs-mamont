use std::io::{Result, Write};

use crate::client::arguments::nfsv3::set_attr::serialize_new_attr;
use crate::interface::vfs::symlink::Args;
use crate::serializer::files::dir_op_arg;
use crate::serializer::files::file_path;

/// Serializes the arguments [`Args`] for an NFSv3 `SYMLINK` operation to the provided `Write` destination.
pub fn symlink_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object)
        .and_then(|_| serialize_new_attr(dest, arg.attr))
        .and_then(|_| file_path(dest, arg.path))
}
