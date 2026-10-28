//! Implements parsing for [`create::Args`] structure.
use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::nfsv3::set_attr::new_attr;
use crate::parser::primitive::{array, u32};
use crate::parser::{Error, Result};
use crate::vfs::create;
use crate::vfs::create::Verifier;

use crate::consts::nfsv3::NFS3_CREATEVERFSIZE;

/// Parses a [`create::How`] enum from the provided `Read` source.
pub fn how(src: &mut impl Read) -> Result<create::How> {
    match u32(src)? {
        0 => Ok(create::How::Unchecked(new_attr(src)?)),
        1 => Ok(create::How::Guarded(new_attr(src)?)),
        2 => Ok(create::How::Exclusive(Verifier(array::<{ NFS3_CREATEVERFSIZE }>(src)?))),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Parses the arguments for an NFSv3 `CREATE` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<create::Args> {
    Ok(create::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        how: how(src)?,
    })
}
