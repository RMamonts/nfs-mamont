use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_name;
use crate::vfs::mk_dir::Args;

use super::set_attr::serialize_new_attr;

pub fn mk_dir_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.dir)
        .and_then(|_| file_name(dest, arg.name))
        .and_then(|_| serialize_new_attr(dest, arg.attr))
}
