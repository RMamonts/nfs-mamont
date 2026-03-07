use std::io::{Result, Write};

use crate::serializer::files::dir_op_arg;
use crate::vfs::rm_dir::Args;

pub fn rm_dir_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object)
}
