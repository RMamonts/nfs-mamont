//! Implements parsing for [`write::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u64;
use crate::parser::primitive::u8;
use crate::parser::primitive::vector;
use crate::parser::Result;
use crate::vfs::write;

fn stable_how(_src: &mut impl Read) -> Result<write::StableHow> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<write::Args> {
    Ok(write::Args {
        file: file::handle(src)?,
        offset: u64(src)?,
        stable: stable_how(src)?,
        data: vector(src, |s| u8(s))?,
    })
}
