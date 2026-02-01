use super::Result;
use crate::parser::primitive::path;
use std::io::Read;
use std::path::PathBuf;

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
pub struct MountArgs(pub PathBuf);

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct UnmountArgs(pub PathBuf);

pub fn mount(src: &mut impl Read) -> Result<MountArgs> {
    Ok(MountArgs(path(src)?))
}

pub fn unmount(src: &mut impl Read) -> Result<UnmountArgs> {
    Ok(UnmountArgs(path(src)?))
}
