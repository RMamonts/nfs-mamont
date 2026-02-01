pub mod access;
pub mod commit;
pub mod create;
pub mod files;
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

pub fn nfs_time(dest: &mut dyn Write, arg: file::Time) -> Result<()> {
    u32(dest, arg.seconds).and_then(|_| u32(dest, arg.nanos))
}
pub fn file_handle(dest: &mut dyn Write, fh: file::Handle) -> Result<()> {
    usize_as_u32(dest, MAX_FILEHANDLE).and_then(|_| array::<MAX_FILEHANDLE>(dest, fh.0))
}

pub fn error(dest: &mut impl Write, stat: vfs::Error) -> Result<()> {
    variant(dest, stat)
}
