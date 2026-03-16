use std::io::{Result, Write};

use crate::interface::vfs::access::Args;
use crate::serializer;
use crate::serializer::files::file_handle;

/// Serializes the arguments [`Args`] for an NFSv3 `ACCESS` operation to the provided `Write` destination.
pub fn access_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file).and_then(|_| serializer::u32(dest, arg.mask.bits()))
}
