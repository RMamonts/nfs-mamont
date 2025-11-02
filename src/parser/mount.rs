use std::io::Read;

use super::Result;
use crate::parser::primitive::string_max_size;
use crate::vfs::{FsPath, MAX_PATH_LEN};

#[allow(dead_code)]
enum MountStat {
    MntOk = 0,
    MntErrPerm = 1,
    MntErrNoEnt = 2,
    MntErrIO = 5,
    MntErrAccess = 13,
    MntErrNotDir = 20,
    MntErrInvalid = 22,
    MntErrNameTooLong = 63,
    MntErrNotSup = 10004,
    MntErrServerFault = 10006,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct MountArgs(pub FsPath);

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct UnmountArgs(pub FsPath);

pub fn mount(src: &mut dyn Read) -> Result<MountArgs> {
    Ok(MountArgs(FsPath(string_max_size(src, MAX_PATH_LEN)?)))
}

pub fn unmount(src: &mut dyn Read) -> Result<UnmountArgs> {
    Ok(UnmountArgs(FsPath(string_max_size(src, MAX_PATH_LEN)?)))
}
