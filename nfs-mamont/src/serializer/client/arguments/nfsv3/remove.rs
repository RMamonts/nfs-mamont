use std::io::{Result, Write};

use crate::serializer::files::dir_op_arg;
use crate::vfs::remove::Args;

/// Serializes the arguments [`Args`] for an NFSv3 `REMOVE` operation to the provided `Write` destination.
pub fn remove_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object)
}
