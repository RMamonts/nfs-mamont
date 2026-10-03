//! Implements [`crate::vfs::file`] structures parsing

use std::io::Read;

use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::parser::primitive::{array, string_max_size, u32, u32_as_usize, u64};
use crate::parser::{Error, Result};
use crate::vfs;
use crate::vfs::file::{Name, Path};
use crate::vfs::{file, MAX_PATH_LEN};

/// Parses a [`file::Handle`] from the provided `Read` source.
pub fn handle(src: &mut impl Read) -> Result<file::Handle> {
    if u32_as_usize(src)? != NFS3_FHSIZE {
        return Err(Error::BadFileHandle);
    }
    let array = array::<{ NFS3_FHSIZE }>(src)?;
    Ok(file::Handle(array))
}

/// Parses a [`file::Type`] from the provided `Read` source.
pub fn r#type(src: &mut impl Read) -> Result<file::Type> {
    use file::Type::*;

    Ok(match u32(src)? {
        1 => Regular,
        2 => Directory,
        3 => BlockDevice,
        4 => CharacterDevice,
        5 => Symlink,
        6 => Socket,
        7 => Fifo,
        _ => return Err(Error::EnumDiscMismatch),
    })
}

/// Parses a [`file::Attr`] structure from the provided `Read` source.
#[allow(dead_code)]
pub fn attr(src: &mut impl Read) -> Result<file::Attr> {
    Ok(file::Attr {
        file_type: r#type(src)?,
        mode: u32(src)?,
        nlink: u32(src)?,
        uid: u32(src)?,
        gid: u32(src)?,
        size: u64(src)?,
        used: u64(src)?,
        device: device(src)?,
        fs_id: u64(src)?,
        file_id: u64(src)?,
        atime: time(src)?,
        mtime: time(src)?,
        ctime: time(src)?,
    })
}

/// Parses a [`file::Time`] structure from the provided `Read` source.
pub fn time(src: &mut impl Read) -> Result<file::Time> {
    Ok(file::Time { seconds: u32(src)?, nanos: u32(src)? })
}

/// Parses a [`file::Device`] structure from the provided `Read` source.
pub fn device(src: &mut impl Read) -> Result<file::Device> {
    Ok(file::Device { major: u32(src)?, minor: u32(src)? })
}

/// Parses a [`file::WccAttr`] structure from the provided `Read` source.
#[allow(dead_code)]
pub fn wcc_attr(src: &mut impl Read) -> Result<file::WccAttr> {
    Ok(file::WccAttr { size: u64(src)?, mtime: time(src)?, ctime: time(src)? })
}

/// Parses a [`file::Name`] structure from the provided `Read` source.
pub fn file_name(src: &mut impl Read) -> Result<file::Name> {
    Name::new(string_max_size(src, vfs::MAX_NAME_LEN)?).map_err(Error::IO)
}

/// Parses a [`file::Path`] structure from the provided `Read` source.
pub fn file_path(src: &mut impl Read) -> Result<file::Path> {
    Path::new(string_max_size(src, MAX_PATH_LEN)?).map_err(Error::IO)
}
