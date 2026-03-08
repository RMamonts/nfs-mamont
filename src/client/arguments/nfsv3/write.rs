use std::io::{Result, Write};

use crate::allocator::Slice;
use crate::serializer::files::file_handle;
use crate::serializer::{padding, u32, u64, usize_as_u32, variant};
use crate::vfs::write::Args;

/// Serializes [`Slice`].
///
/// ## Warning:
/// should be used only in tests
pub fn slice(dest: &mut impl Write, arg: Slice) -> Result<()> {
    let size: usize = arg.iter().map(|buf| buf.len()).sum();
    usize_as_u32(dest, size)?;
    for buf in arg.iter() {
        dest.write_all(buf)?;
    }
    padding(dest, size)?;
    Ok(())
}

/// Serializes the arguments [`Args`] for an NFSv3 `WRITE` operation to the provided `Write` destination.
pub fn write_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
        .and_then(|_| u64(dest, arg.offset))
        .and_then(|_| u32(dest, arg.size))
        .and_then(|_| variant(dest, arg.stable))
        .and_then(|_| slice(dest, arg.data))
}
