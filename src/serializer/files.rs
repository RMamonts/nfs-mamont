//! Shared XDR serializers for common NFSv3 data structures.

use std::io;
use std::io::{ErrorKind, Result, Write};

use crate::serializer::{array, option, string_max_size, u32, u64, usize_as_u32, variant};
use crate::vfs;
use crate::vfs::{file, MAX_PATH_LEN};

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
pub fn wcc_attr(dest: &mut impl Write, wcc: file::WccAttr) -> io::Result<()> {
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
pub fn file_name(dest: &mut impl Write, file_name: file::Name) -> io::Result<()> {
    string_max_size(dest, file_name.into_inner(), vfs::MAX_NAME_LEN)
}

/// Serializes [`FilePath`] as XDR `path` (bounded string).
pub fn file_path(dest: &mut impl Write, file_name: file::Path) -> io::Result<()> {
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
