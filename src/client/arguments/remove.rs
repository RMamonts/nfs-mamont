use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_name;
use crate::vfs::remove::Args;

pub fn remove_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.dir).and_then(|_| file_name(dest, arg.name))
}
