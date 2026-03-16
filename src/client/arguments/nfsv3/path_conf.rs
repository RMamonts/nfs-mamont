use std::io::{Result, Write};

use crate::interface::vfs::path_conf::Args;
use crate::serializer::files::file_handle;

/// Serializes the arguments [`Args`] for an NFSv3 `PATHCONF` operation to the provided `Write` destination.
pub fn path_conf_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
}
