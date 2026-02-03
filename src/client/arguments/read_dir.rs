use std::io::{Result, Write};

use crate::serializer::array;
use crate::serializer::nfs::file_handle;
use crate::serializer::{u32, u64};
use crate::vfs::read_dir::{Args, CookieVerifier, COOKIE_VERF_SIZE};

pub fn serialize_cookie_verifier(dest: &mut impl Write, verifier: CookieVerifier) -> Result<()> {
    array::<COOKIE_VERF_SIZE>(dest, verifier.0)
}

pub fn read_dir_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.dir)
        .and_then(|_| u64(dest, arg.cookie))
        .and_then(|_| serialize_cookie_verifier(dest, arg.cookie_verifier))
        .and_then(|_| u32(dest, arg.count))
}
