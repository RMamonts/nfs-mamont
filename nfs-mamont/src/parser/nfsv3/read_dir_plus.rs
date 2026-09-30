//! Implements parsing for [`read_dir_plus::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::{array, u32, u64};
use crate::parser::Result;
use crate::vfs::{read_dir, read_dir_plus};

/// Parses a [`read_dir::Cookie`] from the provided `Read` source.
pub fn cookie(src: &mut impl Read) -> Result<read_dir::Cookie> {
    Ok(read_dir::Cookie::new(u64(src)?))
}

/// Parses a [`read_dir::CookieVerifier`] from the provided `Read` source.
pub fn cookie_verifier(src: &mut impl Read) -> Result<read_dir::CookieVerifier> {
    Ok(read_dir::CookieVerifier::new(array(src)?))
}

/// Parses the arguments for an NFSv3 `READDIRPLUS` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<read_dir_plus::Args> {
    Ok(read_dir_plus::Args {
        dir: file::handle(src)?,
        cookie: cookie(src)?,
        cookie_verifier: cookie_verifier(src)?,
        dir_count: u32(src)?,
        max_count: u32(src)?,
    })
}
