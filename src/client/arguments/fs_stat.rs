use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::vfs::fs_stat::Args;

pub fn fs_stat_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.root)
}
