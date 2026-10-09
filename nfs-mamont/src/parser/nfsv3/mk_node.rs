//! Implements parsing for [`mk_node::Args`] structure.
use std::io::Read;

use crate::parser::nfsv3::create::new_attr;
use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::primitive::u32;
use crate::parser::{Error, Result};
use crate::vfs::file::Device;
use crate::vfs::mk_node;
use crate::vfs::mk_node::What;

pub(super) fn what(src: &mut impl Read) -> Result<mk_node::What> {
    match u32(src)? {
        3 => Ok(What::Block(new_attr(src)?, Device { major: u32(src)?, minor: u32(src)? })),
        4 => Ok(What::Char(new_attr(src)?, Device { major: u32(src)?, minor: u32(src)? })),
        6 => Ok(What::Socket(new_attr(src)?)),
        7 => Ok(What::Fifo(new_attr(src)?)),
        // NF3REG, NF3DIR and NF3LNK take the `default: void` arm of `mknoddata3`:
        // valid XDR, but RFC 1813 requires NFS3ERR_BADTYPE for them.
        1 | 2 | 5 => Err(Error::BadType),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Parses the arguments for an NFSv3 `MKNOD` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<mk_node::Args> {
    Ok(mk_node::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        what: what(src)?,
    })
}
