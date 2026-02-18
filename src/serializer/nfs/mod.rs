//! NFSv3-specific XDR serializers.
//!
//! Each submodule corresponds to an NFSv3 procedure and provides helpers that
//! serialize the associated `crate::vfs::*` result types into XDR.

pub mod access;
pub mod commit;
pub mod create;
pub mod fs_info;
pub mod fs_stat;
pub mod get_attr;
pub mod link;
pub mod lookup;
pub mod mk_dir;
pub mod mk_node;
pub mod path_conf;
pub mod read;
pub mod read_dir;
pub mod read_dir_plus;
pub mod read_link;
pub mod remove;
pub mod rename;
pub mod rm_dir;
pub mod set_attr;
pub mod symlink;
pub mod write;

use std::io::{Result, Write};

use super::{array, u32, usize_as_u32, variant};
use crate::vfs;
use crate::vfs::file;

const MAX_FILEHANDLE: usize = 8;

/// Serializes `vfs::file::Time` into XDR `nfstime3`.
pub fn nfs_time(dest: &mut dyn Write, arg: file::Time) -> Result<()> {
    u32(dest, arg.seconds).and_then(|_| u32(dest, arg.nanos))
}

/// Serializes `vfs::file::Handle` into XDR `nfs_fh3`.
pub fn file_handle(dest: &mut dyn Write, fh: file::Handle) -> Result<()> {
    usize_as_u32(dest, MAX_FILEHANDLE).and_then(|_| array::<MAX_FILEHANDLE>(dest, fh.0))
}

/// Serializes `vfs::Error` as an XDR enum discriminant (NFS status).
pub fn error(dest: &mut impl Write, stat: vfs::Error) -> Result<()> {
    variant(dest, stat)
}
