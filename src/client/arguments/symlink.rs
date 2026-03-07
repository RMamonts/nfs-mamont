use std::io::{Result, Write};

use super::set_attr::serialize_new_attr;
use crate::serializer::files::dir_op_arg;
use crate::serializer::files::file_path;
use crate::vfs::symlink::Args;

pub fn symlink_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object)
        .and_then(|_| serialize_new_attr(dest, arg.attr))
        .and_then(|_| file_path(dest, arg.path))
}
