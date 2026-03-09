use std::io::{Result, Write};

use crate::serializer::files::file_handle;
use crate::vfs::path_conf::Args;

/// Serializes the arguments [`Args`] for an NFSv3 `PATHCONF` operation to the provided `Write` destination.
pub fn path_conf_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
}
