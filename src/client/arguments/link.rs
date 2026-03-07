use std::io::{Result, Write};

use crate::serializer::files::dir_op_arg;
use crate::serializer::files::file_handle;
use crate::vfs::link::Args;

pub fn link_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file).and_then(|_| dir_op_arg(dest, arg.link))
}
