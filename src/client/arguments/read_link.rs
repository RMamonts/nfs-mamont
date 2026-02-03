use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::vfs::read_link::Args;

pub fn read_link_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
}
