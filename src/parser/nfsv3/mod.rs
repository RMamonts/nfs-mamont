pub mod procedures;

use std::io::Read;

use crate::nfsv3::{
    createhow3, devicedata3, diropargs3, mknoddata3, nfs_fh3, nfstime3, sattr3, set_atime,
    set_mtime, specdata3, symlinkdata3, NFS3_CREATEVERFSIZE,
};
use crate::parser::primitive::{
    array, option, string_max_size, u32, u64, u8, u32_as_usize,
};
use crate::parser::{Error, Result};
use crate::vfs::{FileHandle, FileName};

#[allow(dead_code)]
const MAX_FILENAME: usize = 255;
#[allow(dead_code)]
pub const MAX_FILEHANDLE: usize = 8;
#[allow(dead_code)]
const MAX_FILEPATH: usize = 1024;

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct DirOpArg {
    pub object: FileHandle,
    pub name: FileName,
}

#[allow(dead_code)]
pub fn specdata3(src: &mut dyn Read) -> Result<specdata3> {
    Ok(specdata3 { specdata1: u32(src)?, specdata2: u32(src)? })
}

pub fn nfstime(src: &mut dyn Read) -> Result<nfstime3> {
    Ok(nfstime3 { seconds: u32(src)?, nseconds: u32(src)? })
}

#[allow(dead_code)]
pub fn set_atime(src: &mut dyn Read) -> Result<set_atime> {
    match u32(src)? {
        0 => Ok(set_atime::DONT_CHANGE),
        1 => Ok(set_atime::SET_TO_SERVER_TIME),
        2 => Ok(set_atime::SET_TO_CLIENT_TIME(nfstime(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn set_mtime(src: &mut dyn Read) -> Result<set_mtime> {
    match u32(src)? {
        0 => Ok(set_mtime::DONT_CHANGE),
        1 => Ok(set_mtime::SET_TO_SERVER_TIME),
        2 => Ok(set_mtime::SET_TO_CLIENT_TIME(nfstime(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn sattr3(src: &mut dyn Read) -> Result<sattr3> {
    Ok(sattr3 {
        mode: option(src, |s| u32(s))?,
        uid: option(src, |s| u32(s))?,
        gid: option(src, |s| u32(s))?,
        size: option(src, |s| u64(s))?,
        atime: set_atime(src)?,
        mtime: set_mtime(src)?,
    })
}

#[allow(dead_code)]
pub fn nfs_fh3(src: &mut dyn Read) -> Result<nfs_fh3> {
    let size = u32_as_usize(src)?;
    if size != MAX_FILEHANDLE {
        return Err(Error::BadFileHandle);
    }
    Ok(nfs_fh3 { data: array::<MAX_FILEHANDLE, u8>(src, |s| u8(s))? })
}

#[allow(dead_code)]
pub fn diropargs3(src: &mut dyn Read) -> Result<diropargs3> {
    Ok(diropargs3 { dir: nfs_fh3(src)?, name: string_max_size(src, MAX_FILEPATH)? })
}

#[allow(dead_code)]
pub fn createhow3(src: &mut dyn Read) -> Result<createhow3> {
    match u32(src)? {
        0 => Ok(createhow3::UNCHECKED(sattr3(src)?)),
        1 => Ok(createhow3::GUARDED(sattr3(src)?)),
        2 => Ok(createhow3::EXCLUSIVE(array::<NFS3_CREATEVERFSIZE, u8>(src, |s| u8(s))?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn symlinkdata3(src: &mut dyn Read) -> Result<symlinkdata3> {
    Ok(symlinkdata3 {
        symlink_attributes: sattr3(src)?,
        symlink_data: string_max_size(src, MAX_FILEPATH)?,
    })
}

#[allow(dead_code)]
pub fn devicedata3(src: &mut dyn Read) -> Result<devicedata3> {
    Ok(devicedata3 { dev_attributes: sattr3(src)?, spec: specdata3(src)? })
}

#[allow(dead_code)]
pub fn mknoddata3(src: &mut dyn Read) -> Result<mknoddata3> {
    match u32(src)? {
        1 => Ok(mknoddata3::NF3REG),
        2 => Ok(mknoddata3::NF3DIR),
        3 => Ok(mknoddata3::NF3BLK(devicedata3(src)?)),
        4 => Ok(mknoddata3::NF3CHR(devicedata3(src)?)),
        5 => Ok(mknoddata3::NF3LNK),
        6 => Ok(mknoddata3::NF3SOCK(sattr3(src)?)),
        7 => Ok(mknoddata3::NF3FIFO(sattr3(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}
