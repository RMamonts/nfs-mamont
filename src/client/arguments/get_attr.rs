use std::io::{Result, Write};

use crate::serializer::files::file_handle;
use crate::vfs::get_attr::Args;

/// Serializes the arguments [`Args`] for an NFSv3 `GETATTR` operation to the provided `Write` destination.
pub fn get_attr_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
}
