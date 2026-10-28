//! Implements parsing for [`create::Args`] structure.
use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::primitive::{array, option, u32, u64, variant};
use crate::parser::Result;
use crate::vfs::create;
use crate::vfs::create::{HowMode, Verifier};
use crate::vfs::file::Time;
use crate::vfs::set_attr::{NewAttr, SetTime, SetTimeMode};

use crate::consts::nfsv3::NFS3_CREATEVERFSIZE;

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
        SetTimeMode::ToClient => Ok(SetTime::ToClient(nfs_time(src)?)),
    }
}

/// Parses an NFS time structure from the provided `Read` source.
pub fn nfs_time(src: &mut impl Read) -> Result<Time> {
    Ok(Time { seconds: u32(src)?, nanos: u32(src)? })
}

/// Parses a [`create::How`] enum from the provided `Read` source.
pub fn how(src: &mut impl Read) -> Result<create::How> {
    match variant::<HowMode>(src)? {
        HowMode::Unchecked => Ok(create::How::Unchecked(new_attr(src)?)),
        HowMode::Guarded => Ok(create::How::Guarded(new_attr(src)?)),
        HowMode::Exclusive => {
            Ok(create::How::Exclusive(Verifier(array::<{ NFS3_CREATEVERFSIZE }>(src)?)))
        }
    }
}

/// Parses the arguments for an NFSv3 `CREATE` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<create::Args> {
    Ok(create::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        how: how(src)?,
    })
}
