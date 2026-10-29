//! Implements parsing for [`mk_node::Args`] structure.
use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::{device, file_name};
use crate::parser::nfsv3::set_attr::new_attr;
use crate::parser::primitive::variant;
use crate::parser::{Error, Result};
use crate::vfs::file::Type;
use crate::vfs::mk_node;
use crate::vfs::mk_node::What;

/// Parses a [`mk_node::What`] union from the provided `Read` source.
///
/// The discriminant is a [`Type`]; only the special-file types are valid here.
pub fn what(src: &mut impl Read) -> Result<mk_node::What> {
    match variant::<Type>(src)? {
        Type::BlockDevice => Ok(What::Block(new_attr(src)?, device(src)?)),
        Type::CharacterDevice => Ok(What::Char(new_attr(src)?, device(src)?)),
        Type::Socket => Ok(What::Socket(new_attr(src)?)),
        Type::Fifo => Ok(What::Fifo(new_attr(src)?)),
        Type::Regular | Type::Directory | Type::Symlink => Err(Error::EnumDiscMismatch),
    }
}

/// Parses the arguments for an NFSv3 `MKNOD` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<mk_node::Args> {
    Ok(mk_node::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        what: what(src)?,
    })
}
