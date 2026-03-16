use std::io::{Result, Write};

use crate::interface::vfs::commit::Args;
use crate::serializer::files::file_handle;
use crate::serializer::{u32, u64};

/// Serializes the arguments [`Args`] for an NFSv3 `COMMIT` operation to the provided `Write` destination.
pub fn commit_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
        .and_then(|_| u64(dest, arg.offset))
        .and_then(|_| u32(dest, arg.count))
}
