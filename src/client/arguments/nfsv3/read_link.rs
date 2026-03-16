use std::io::{Result, Write};

use crate::interface::vfs::read_link::Args;
use crate::serializer::files::file_handle;

/// Serializes the arguments [`Args`] for an NFSv3 `READLINK` operation to the provided `Write` destination.
pub fn read_link_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
}
