use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::vfs::get_attr::Args;

pub fn get_attr_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
}
