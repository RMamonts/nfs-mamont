use std::io::{Result, Write};

use crate::interface::vfs::rename::Args;
use crate::serializer::files::dir_op_arg;

/// Serializes the arguments [`Args`] for an NFSv3 `RENAME` operation to the provided `Write` destination.
pub fn rename_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.from).and_then(|_| dir_op_arg(dest, arg.to))
}
