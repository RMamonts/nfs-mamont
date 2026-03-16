use std::io::{Result, Write};

use crate::interface::vfs::link::Args;
use crate::serializer::files::dir_op_arg;
use crate::serializer::files::file_handle;

/// Serializes the arguments [`Args`] for an NFSv3 `LINK` operation to the provided `Write` destination.
pub fn link_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file).and_then(|_| dir_op_arg(dest, arg.link))
}
