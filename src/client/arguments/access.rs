use std::io::{Result, Write};

use crate::serializer;
use crate::serializer::files::file_handle;
use crate::vfs::access::Args;

pub fn access_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file).and_then(|_| serializer::u32(dest, arg.mask.bits()))
}
