//! Implements parsing for [`read_dir_plus::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u64;
use crate::parser::Result;
use crate::vfs::read_dir;
use crate::vfs::read_dir_plus;

pub fn cookie(src: &mut impl Read) -> Result<read_dir::Cookie> {
    todo!()
}

pub fn cookie_verifier(src: &mut impl Read) -> Result<read_dir::CookieVerifier> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<read_dir_plus::Args> {
    Ok(read_dir_plus::Args {
        dir: file::handle(src)?,
        cookie: cookie(src)?,
        cookie_verifier: cookie_verifier(src)?,
        dir_count: u64(src)?,
        max_count: u64(src)?,
    })
}
