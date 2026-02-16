//! Parses MOUNT protocol operations.
use std::io::Read;

use crate::parser::nfsv3::file::file_path;
use crate::vfs::file;

use super::Result;

/// Represents the status codes returned by MOUNT operations.
#[allow(dead_code)]
enum MountStat {
    /// The operation completed successfully.
    MntOk = 0,
    /// Permission denied.
    MntErrPerm = 1,
    /// No such file or directory.
    MntErrNoEnt = 2,
    /// I/O error.
    MntErrIO = 5,
    /// Access denied.
    MntErrAccess = 13,
    /// Not a directory.
    MntErrNotDir = 20,
    /// Invalid argument.
    MntErrInvalid = 22,
    /// Name too long.
    MntErrNameTooLong = 63,
    /// Operation not supported.
    MntErrNotSup = 10004,
    /// A server fault occurred.
    MntErrServerFault = 10006,
}

/// Arguments for the Mount operation, containing the path to be mounted.
#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct MountArgs(pub file::FilePath);

/// Arguments for the Unmount operation, containing the path to be unmounted.
#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct UnmountArgs(pub file::FilePath);

/// Parses the arguments for a Mount operation.
pub fn mount(src: &mut impl Read) -> Result<MountArgs> {
    Ok(MountArgs(file_path(src)?))
}

/// Parses the arguments for an Unmount operation.
pub fn unmount(src: &mut impl Read) -> Result<UnmountArgs> {
    Ok(UnmountArgs(file_path(src)?))
}
