use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::{u32, u64};
use crate::vfs::read_dir_plus::Args;

use super::read_dir::serialize_cookie_verifier;

pub fn read_dir_plus_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.dir)
        .and_then(|_| u64(dest, arg.cookie))
        .and_then(|_| serialize_cookie_verifier(dest, arg.cookie_verifier))
        .and_then(|_| u32(dest, arg.dir_count))
        .and_then(|_| u32(dest, arg.max_count))
}
