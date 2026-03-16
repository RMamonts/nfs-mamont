use std::io::{Result, Write};

use crate::interface::vfs::file::{Device, Type};
use crate::interface::vfs::mk_node::{Args, What};
use crate::serializer::files::dir_op_arg;
use crate::serializer::u32;

use crate::client::arguments::nfsv3::set_attr::serialize_new_attr;

/// Serializes [`Device`].
fn serialize_device(dest: &mut impl Write, arg: Device) -> Result<()> {
    u32(dest, arg.major).and_then(|_| u32(dest, arg.minor))
}

/// Serializes [`What`].
fn serialize_how(dest: &mut impl Write, what: What) -> Result<()> {
    match what {
        What::Char(attr, fh) => u32(dest, Type::CharacterDevice as u32)
            .and_then(|_| serialize_new_attr(dest, attr))
            .and_then(|_| serialize_device(dest, fh)),
        What::Block(attr, fh) => u32(dest, Type::BlockDevice as u32)
            .and_then(|_| serialize_new_attr(dest, attr))
            .and_then(|_| serialize_device(dest, fh)),
        What::Socket(attr) => {
            u32(dest, Type::Socket as u32).and_then(|_| serialize_new_attr(dest, attr))
        }
        What::Fifo(attr) => {
            u32(dest, Type::Fifo as u32).and_then(|_| serialize_new_attr(dest, attr))
        }
        What::Regular => u32(dest, Type::Regular as u32),
        What::Directory => u32(dest, Type::Directory as u32),
        What::SymbolicLink => u32(dest, Type::Symlink as u32),
    }
}

/// Serializes the arguments [`Args`] for an NFSv3 `MKNOD` operation to the provided `Write` destination.
pub fn mk_node_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object).and_then(|_| serialize_how(dest, arg.what))
}
