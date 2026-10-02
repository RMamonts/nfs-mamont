//! Implements parsing for [`set_attr::Args`] structure.

use std::io::Read;

pub use crate::parser::nfsv3::create::{new_attr, nfs_time};
use crate::parser::nfsv3::file;
use crate::parser::primitive::bool;
use crate::parser::Result;
use crate::vfs::set_attr;
use crate::vfs::set_attr::Guard;

/// Parses an optional [`set_attr::Guard`] structure from the provided `Read` source.
pub fn guard(src: &mut impl Read) -> Result<Option<set_attr::Guard>> {
    match bool(src)? {
        true => Ok(Some(Guard { ctime: nfs_time(src)? })),
        false => Ok(None),
    }
}

/// Parses the arguments for an NFSv3 `SETATTR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<set_attr::Args> {
    Ok(set_attr::Args { file: file::handle(src)?, new_attr: new_attr(src)?, guard: guard(src)? })
}
