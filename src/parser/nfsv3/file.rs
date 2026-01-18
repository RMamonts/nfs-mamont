//! Implements [`crate::vfs::file`] structures parsing

use std::io::Read;

use crate::parser::primitive::{array, option, u32, u32_as_usize, u64, u8};
use crate::parser::{Error, Result};
use crate::vfs::file;

pub fn handle(src: &mut dyn Read) -> Result<file::Handle> {
    if u32_as_usize(src)? != file::HANDLE_SIZE {
        return Err(Error::BadFileHandle);
    }
    let array = array::<{ file::HANDLE_SIZE }, u8>(src, |s| u8(s))?;
    Ok(file::Handle(array))
}

pub fn r#type(src: &mut dyn Read) -> Result<file::Type> {
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

pub fn attr(src: &mut impl Read) -> Result<file::Attr> {
    Ok(file::Attr {
        file_type: r#type(src)?,
        mode: u32(src)?,
        nlink: u32(src)?,
        uid: u32(src)?,
        gid: u32(src)?,
        size: u64(src)?,
        used: u64(src)?,
        device: option(src, |s| device(s))?,
        fs_id: u64(src)?,
        file_id: u64(src)?,
        atime: time(src)?,
        mtime: time(src)?,
        ctime: time(src)?,
    })
}

pub fn time(src: &mut impl Read) -> Result<file::Time> {
    Ok(file::Time { seconds: u32(src)?, nanos: u32(src)? })
}

pub fn device(src: &mut impl Read) -> Result<file::Device> {
    Ok(file::Device { major: u32(src)?, minor: u32(src)? })
}

pub fn wcc_attr(src: &mut impl Read) -> Result<file::WccAttr> {
    Ok(file::WccAttr { size: u64(src)?, mtime: time(src)?, ctime: time(src)? })
}
