use std::io::Read;

use crate::nfsv3::{
    createhow3, devicedata3, diropargs3, mknoddata3, nfs_fh3, nfstime3, sattr3, set_atime,
    set_mtime, specdata3, symlinkdata3, NFS3_CREATEVERFSIZE,
};
use crate::parser::primitive::{
    parse_array, parse_option, parse_string_max_len, parse_u32, parse_u64, parse_u8,
};
use crate::parser::Error;

#[allow(dead_code)]
const MAX_FILENAME: usize = 255;
#[allow(dead_code)]
pub const MAX_FILEHANDLE: usize = 8;
#[allow(dead_code)]
const MAX_FILEPATH: usize = 255;

#[allow(dead_code)]
pub fn parse_specdata3(src: &mut dyn Read) -> Result<specdata3, Error> {
    Ok(specdata3 { specdata1: parse_u32(src)?, specdata2: parse_u32(src)? })
}

pub fn parse_nfstime(src: &mut dyn Read) -> Result<nfstime3, Error> {
    Ok(nfstime3 { seconds: parse_u32(src)?, nseconds: parse_u32(src)? })
}

#[allow(dead_code)]
pub fn parse_set_atime(src: &mut dyn Read) -> Result<set_atime, Error> {
    let disc = parse_u32(src)?;
    match disc {
        0 => Ok(set_atime::DONT_CHANGE),
        1 => Ok(set_atime::SET_TO_SERVER_TIME),
        2 => Ok(set_atime::SET_TO_CLIENT_TIME(parse_nfstime(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn parse_set_mtime(src: &mut dyn Read) -> Result<set_mtime, Error> {
    let disc = parse_u32(src)?;
    match disc {
        0 => Ok(set_mtime::DONT_CHANGE),
        1 => Ok(set_mtime::SET_TO_SERVER_TIME),
        2 => Ok(set_mtime::SET_TO_CLIENT_TIME(parse_nfstime(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn parse_sattr3(src: &mut dyn Read) -> Result<sattr3, Error> {
    Ok(sattr3 {
        mode: parse_option(src, |s| parse_u32(s))?,
        uid: parse_option(src, |s| parse_u32(s))?,
        gid: parse_option(src, |s| parse_u32(s))?,
        size: parse_option(src, |s| parse_u64(s))?,
        atime: parse_set_atime(src)?,
        mtime: parse_set_mtime(src)?,
    })
}

#[allow(dead_code)]
pub fn parse_nfs_fh3(src: &mut dyn Read) -> Result<nfs_fh3, Error> {
    Ok(nfs_fh3 { data: parse_array::<MAX_FILEHANDLE, u8>(src, |s| parse_u8(s))? })
}

#[allow(dead_code)]
pub fn parse_diropargs3(src: &mut dyn Read) -> Result<diropargs3, Error> {
    Ok(diropargs3 { dir: parse_nfs_fh3(src)?, name: parse_string_max_len(src, MAX_FILEPATH)? })
}

#[allow(dead_code)]
pub fn parse_createhow3(src: &mut dyn Read) -> Result<createhow3, Error> {
    let disc = parse_u32(src)?;
    match disc {
        0 => Ok(createhow3::UNCHECKED(parse_sattr3(src)?)),
        1 => Ok(createhow3::UNCHECKED(parse_sattr3(src)?)),
        2 => {
            Ok(createhow3::EXCLUSIVE(parse_array::<NFS3_CREATEVERFSIZE, u8>(src, |s| parse_u8(s))?))
        }
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn parse_symlinkdata3(src: &mut dyn Read) -> Result<symlinkdata3, Error> {
    Ok(symlinkdata3 {
        symlink_attributes: parse_sattr3(src)?,
        symlink_data: parse_string_max_len(src, MAX_FILEPATH)?,
    })
}

#[allow(dead_code)]
pub fn parse_devicedata3(src: &mut dyn Read) -> Result<devicedata3, Error> {
    Ok(devicedata3 { dev_attributes: parse_sattr3(src)?, spec: parse_specdata3(src)? })
}

#[allow(dead_code)]
pub fn parse_mknoddata3(src: &mut dyn Read) -> Result<mknoddata3, Error> {
    let disc = parse_u32(src)?;
    match disc {
        1 => Ok(mknoddata3::NF3REG),
        2 => Ok(mknoddata3::NF3DIR),
        3 => Ok(mknoddata3::NF3BLK(parse_devicedata3(src)?)),
        4 => Ok(mknoddata3::NF3CHR(parse_devicedata3(src)?)),
        5 => Ok(mknoddata3::NF3LNK),
        6 => Ok(mknoddata3::NF3SOCK(parse_sattr3(src)?)),
        7 => Ok(mknoddata3::NF3FIFO(parse_sattr3(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}
