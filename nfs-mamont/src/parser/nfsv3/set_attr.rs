//! Implements parsing for [`set_attr::Args`] and the shared [`NewAttr`]/[`SetTime`] structures.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::{bool, option, u32, u64, variant};
use crate::parser::Result;
use crate::vfs::set_attr;
use crate::vfs::set_attr::{Guard, NewAttr, SetTime, SetTimeMode};

/// Parses a [`NewAttr`] structure from the provided `Read` source.
pub fn new_attr(src: &mut impl Read) -> Result<NewAttr> {
    Ok(NewAttr {
        mode: option(src, u32)?,
        uid: option(src, u32)?,
        gid: option(src, u32)?,
        size: option(src, u64)?,
        atime: set_time(src)?,
        mtime: set_time(src)?,
    })
}

/// Parses a [`SetTime`] enum from the provided `Read` source.
pub fn set_time(src: &mut impl Read) -> Result<SetTime> {
    match variant::<SetTimeMode>(src)? {
        SetTimeMode::DontChange => Ok(SetTime::DontChange),
        SetTimeMode::ToServer => Ok(SetTime::ToServer),
        SetTimeMode::ToClient => Ok(SetTime::ToClient(file::time(src)?)),
    }
}

/// Parses an optional [`set_attr::Guard`] structure from the provided `Read` source.
pub fn guard(src: &mut impl Read) -> Result<Option<set_attr::Guard>> {
    match bool(src)? {
        true => Ok(Some(Guard { ctime: file::time(src)? })),
        false => Ok(None),
    }
}

/// Parses the arguments for an NFSv3 `SETATTR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<set_attr::Args> {
    Ok(set_attr::Args { file: file::handle(src)?, new_attr: new_attr(src)?, guard: guard(src)? })
}
