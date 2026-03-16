use std::io::{Result, Write};

use crate::interface::vfs::fs_stat::Args;
use crate::serializer::files::file_handle;

/// Serializes the arguments [`Args`] for an NFSv3 `FSSTAT` operation to the provided `Write` destination.
pub fn fs_stat_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.root)
}
