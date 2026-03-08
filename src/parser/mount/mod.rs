//! Parses MOUNT protocol operations.

pub mod mnt;
pub mod umnt;

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
