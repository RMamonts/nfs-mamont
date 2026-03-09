use std::io::{Result, Write};

use crate::serializer::files::file_handle;
use crate::vfs::fs_stat::Args;

/// Serializes the arguments [`Args`] for an NFSv3 `FSSTAT` operation to the provided `Write` destination.
pub fn fs_stat_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.root)
}
