use std::io::{Result, Write};

use crate::serializer::files::file_handle;
use crate::vfs::fs_info::Args;

pub fn fs_info_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.root)
}
