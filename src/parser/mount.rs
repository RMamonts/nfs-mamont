use std::io::Read;

use super::Result;
use crate::parser::nfsv3::file::file_path;
use crate::vfs::file::FilePath;

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
pub struct MountArgs(pub FilePath);

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct UnmountArgs(pub FilePath);

pub fn mount(src: &mut impl Read) -> Result<MountArgs> {
    Ok(MountArgs(file_path(src)?))
}

pub fn unmount(src: &mut impl Read) -> Result<UnmountArgs> {
    Ok(UnmountArgs(file_path(src)?))
}
