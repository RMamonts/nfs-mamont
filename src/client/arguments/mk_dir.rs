use std::io::{Result, Write};

use crate::serializer::files::dir_op_arg;
use crate::vfs::mk_dir::Args;

use super::set_attr::serialize_new_attr;

pub fn mk_dir_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object).and_then(|_| serialize_new_attr(dest, arg.attr))
}
