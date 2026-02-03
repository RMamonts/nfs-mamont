use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::{u32, u64};
use crate::vfs::commit::Args;

pub fn commit_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
        .and_then(|_| u64(dest, arg.offset))
        .and_then(|_| u32(dest, arg.count))
}
