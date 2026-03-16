use std::io::{Result, Write};

use crate::interface::vfs::get_attr::Args;
use crate::serializer::files::file_handle;

/// Serializes the arguments [`Args`] for an NFSv3 `GETATTR` operation to the provided `Write` destination.
pub fn get_attr_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
}
