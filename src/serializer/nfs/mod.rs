pub mod results;

use std::io::{Result, Write};

use crate::nfsv3::{
    createhow3, devicedata3, mknoddata3, nfs_fh3, nfstime3, sattr3, set_atime, set_mtime, specdata3,
};

use super::{array, option, string_max_size, u32, u64, usize_as_u32};

const MAX_FILEHANDLE: usize = 8;

const MAX_FILEPATH: usize = 1024;

pub fn specdata3(dest: &mut dyn Write, arg: specdata3) -> Result<()> {
    u32(dest, arg.specdata1).and_then(|_| u32(dest, arg.specdata2))
}

pub fn nfstime(dest: &mut dyn Write, arg: nfstime3) -> Result<()> {
    u32(dest, arg.seconds).and_then(|_| u32(dest, arg.nseconds))
}

#[allow(dead_code)]
pub fn set_atime(dest: &mut dyn Write, arg: set_atime) -> Result<()> {
    match arg {
        set_atime::DONT_CHANGE => u32(dest, 0),
        set_atime::SET_TO_SERVER_TIME => u32(dest, 1),
        set_atime::SET_TO_CLIENT_TIME(time) => u32(dest, 2).and_then(|_| nfstime(dest, time)),
    }
}

#[allow(dead_code)]
pub fn set_mtime(dest: &mut dyn Write, arg: set_mtime) -> Result<()> {
    match arg {
        set_mtime::DONT_CHANGE => u32(dest, 0),
        set_mtime::SET_TO_SERVER_TIME => u32(dest, 1),
        set_mtime::SET_TO_CLIENT_TIME(time) => u32(dest, 2).and_then(|_| nfstime(dest, time)),
    }
}

pub fn sattr3(dest: &mut dyn Write, arg: sattr3) -> Result<()> {
    option(dest, arg.mode, |t, dest| u32(dest, t))?;
    option(dest, arg.uid, |t, dest| u32(dest, t))?;
    option(dest, arg.gid, |t, dest| u32(dest, t))?;
    option(dest, arg.size, |t, dest| u64(dest, t))?;
    set_atime(dest, arg.atime)?;
    set_mtime(dest, arg.mtime)?;
    Ok(())
}

pub fn nfs_fh3(dest: &mut dyn Write, fh: nfs_fh3) -> Result<()> {
    usize_as_u32(dest, MAX_FILEHANDLE).and_then(|_| array::<MAX_FILEHANDLE>(dest, fh.data))
}

#[allow(dead_code)]
pub fn diropargs3(dest: &mut dyn Write, fh: nfs_fh3, name: String) -> Result<()> {
    nfs_fh3(dest, fh).and_then(|_| string_max_size(dest, name, MAX_FILEPATH))
}

#[allow(dead_code)]
pub fn createhow3(dest: &mut dyn Write, arg: createhow3) -> Result<()> {
    match arg {
        createhow3::UNCHECKED(sattr) => u32(dest, 0).and_then(|_| sattr3(dest, sattr)),
        createhow3::GUARDED(sattr) => u32(dest, 1).and_then(|_| sattr3(dest, sattr)),
        createhow3::EXCLUSIVE(arr) => u32(dest, 2).and_then(|_| array(dest, arr)),
    }
}

#[allow(dead_code)]
pub fn symlinkdata3(dest: &mut dyn Write, sattr: sattr3, path: String) -> Result<()> {
    sattr3(dest, sattr).and_then(|_| string_max_size(dest, path, MAX_FILEPATH))
}

pub fn devicedata3(dest: &mut dyn Write, devicedata: devicedata3) -> Result<()> {
    sattr3(dest, devicedata.dev_attributes).and_then(|_| specdata3(dest, devicedata.spec))
}

#[allow(dead_code)]
pub fn mknoddata3(dest: &mut dyn Write, arg: mknoddata3) -> Result<()> {
    match arg {
        mknoddata3::NF3REG => u32(dest, 1),
        mknoddata3::NF3DIR => u32(dest, 2),
        mknoddata3::NF3BLK(dev) => u32(dest, 3).and_then(|_| devicedata3(dest, dev)),
        mknoddata3::NF3CHR(dev) => u32(dest, 4).and_then(|_| devicedata3(dest, dev)),
        mknoddata3::NF3LNK => u32(dest, 5),
        mknoddata3::NF3SOCK(sattr) => u32(dest, 6).and_then(|_| sattr3(dest, sattr)),
        mknoddata3::NF3FIFO(sattr) => u32(dest, 7).and_then(|_| sattr3(dest, sattr)),
    }
}
