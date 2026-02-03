use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_name;
use crate::vfs::rename::Args;

pub fn rename_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.from_dir)
        .and_then(|_| file_name(dest, arg.from_name))
        .and_then(|_| file_handle(dest, arg.to_dir))
        .and_then(|_| file_name(dest, arg.to_name))
}
