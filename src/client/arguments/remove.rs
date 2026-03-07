use std::io::{Result, Write};

use crate::serializer::files::dir_op_arg;
use crate::vfs::remove::Args;

pub fn remove_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object)
}
