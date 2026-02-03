use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_name;
use crate::vfs::lookup::Args;

pub fn lookup_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.parent).and_then(|_| file_name(dest, arg.name))
}
