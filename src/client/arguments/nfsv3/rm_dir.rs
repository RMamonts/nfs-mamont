use std::io::{Result, Write};

use crate::interface::vfs::rm_dir::Args;
use crate::serializer::files::dir_op_arg;

/// Serializes the arguments [`Args`] for an NFSv3 `RMDIR` operation to the provided `Write` destination.
pub fn rm_dir_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object)
}
