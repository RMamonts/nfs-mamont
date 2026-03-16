use std::io::{Result, Write};

use crate::interface::vfs::lookup::Args;
use crate::serializer::files::file_handle;
use crate::serializer::files::file_name;

/// Serializes the arguments [`Args`] for an NFSv3 `LOOKUP` operation to the provided `Write` destination.
pub fn lookup_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.parent).and_then(|_| file_name(dest, arg.name))
}
