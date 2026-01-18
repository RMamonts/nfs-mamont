//! Implements parsing for [`read_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::read_dir_plus::cookie;
use crate::parser::nfsv3::read_dir_plus::cookie_verifier;
use crate::parser::primitive::u64;
use crate::parser::Result;
use crate::vfs::read_dir;

pub fn args(src: &mut impl Read) -> Result<read_dir::Args> {
    Ok(read_dir::Args {
        dir: file::handle(src)?,
        cookie: cookie(src)?,
        cookie_verifier: cookie_verifier(src)?,
        count: u64(src)?,
    })
}
