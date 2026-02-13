//! Shared XDR serializers for common NFSv3 data structures.

use std::io;
use std::io::{ErrorKind, Write};

use crate::serializer::nfs::nfs_time;
use crate::serializer::{option, string_max_size, u32, u64, variant};
use crate::vfs;
use crate::vfs::file::{FileName, FilePath};
use crate::vfs::{file, MAX_PATH_LEN};

/// Serializes [file::Type] as the XDR `ftype3` enum discriminant.
pub fn file_type(dest: &mut impl Write, file_type: file::Type) -> io::Result<()> {
    variant::<file::Type>(dest, file_type)
}

/// Serializes [`file::Attr`] as XDR `fattr3` (file attributes).
pub fn file_attr(dest: &mut impl Write, attr: &file::Attr) -> io::Result<()> {
    file_type(dest, attr.file_type)?;
    u32(dest, attr.mode)?;
    u32(dest, attr.nlink)?;
    u32(dest, attr.uid)?;
    u32(dest, attr.gid)?;
    u64(dest, attr.size)?;
    u64(dest, attr.used)?;
    u32(dest, attr.device.major)?;
    u32(dest, attr.device.minor)?;
    u64(dest, attr.fs_id)?;
    u64(dest, attr.file_id)?;
    nfs_time(dest, attr.atime)?;
    nfs_time(dest, attr.mtime)?;
    nfs_time(dest, attr.ctime)
}

/// Serializes [`file::WccAttr`] as XDR `wcc_attr` (weak cache consistency).
pub fn wcc_attr(dest: &mut dyn Write, wcc: file::WccAttr) -> io::Result<()> {
    u64(dest, wcc.size)?;
    nfs_time(dest, wcc.mtime)?;
    nfs_time(dest, wcc.ctime)
}

/// Serializes [`vfs::WccData`] as XDR `wcc_data` (before/after attributes).
pub fn wcc_data(dest: &mut impl Write, wcc: vfs::WccData) -> io::Result<()> {
    option(dest, wcc.before, |attr, dest| wcc_attr(dest, attr))?;
    option(dest, wcc.after, |attr, dest| file_attr(dest, &attr))
}

/// Serializes [`FileName`] as XDR `filename3` (bounded string).
pub fn file_name(dest: &mut impl Write, file_name: FileName) -> io::Result<()> {
    string_max_size(dest, file_name.into_inner(), vfs::MAX_NAME_LEN)
}

/// Serializes [`FilePath`] as XDR `path` (bounded string).
pub fn file_path(dest: &mut impl Write, file_name: FilePath) -> io::Result<()> {
    string_max_size(
        dest,
        file_name
            .into_inner()
            .into_os_string()
            .into_string()
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid path"))?,
        MAX_PATH_LEN,
    )
}
