use crate::parser::nfsv3::MAX_FILENAME;
use crate::serializer::nfs::nfs_time;
use crate::serializer::{option, string_max_size, u32, u64, variant};
use crate::vfs;
use crate::vfs::file::{FileName, FilePath};
use crate::vfs::{file, MAX_PATH_LEN};
use std::io;
use std::io::{ErrorKind, Write};

pub fn file_type<S: Write>(dest: &mut S, file_type: file::Type) -> io::Result<()> {
    variant::<file::Type, S>(dest, file_type)
}

pub fn file_attr<S: Write>(dest: &mut S, attr: file::Attr) -> io::Result<()> {
    file_type(dest, attr.file_type)?;
    u32(dest, attr.mode)?;
    u32(dest, attr.nlink)?;
    u32(dest, attr.uid)?;
    u32(dest, attr.gid)?;
    u64(dest, attr.size)?;
    u64(dest, attr.used)?;
    u32(dest, attr.device.minor)?;
    u32(dest, attr.device.major)?;
    u64(dest, attr.fs_id)?;
    u64(dest, attr.file_id)?;
    nfs_time(dest, attr.atime)?;
    nfs_time(dest, attr.mtime)?;
    nfs_time(dest, attr.ctime)
}

pub fn wcc_attr(dest: &mut dyn Write, wcc: file::WccAttr) -> io::Result<()> {
    u64(dest, wcc.size)?;
    nfs_time(dest, wcc.mtime)?;
    nfs_time(dest, wcc.ctime)
}

pub fn wcc_data<S: Write>(dest: &mut S, wcc: vfs::WccData) -> io::Result<()> {
    option(dest, wcc.before, |attr, dest| wcc_attr(dest, attr))?;
    option(dest, wcc.after, |attr, dest| file_attr(dest, attr))
}

pub fn file_name(dest: &mut impl Write, file_name: FileName) -> io::Result<()> {
    string_max_size(dest, file_name.0, MAX_FILENAME)
}

pub fn file_path(dest: &mut impl Write, file_name: FilePath) -> io::Result<()> {
    string_max_size(
        dest,
        file_name
            .0
            .into_os_string()
            .into_string()
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid path"))?,
        MAX_PATH_LEN,
    )
}
