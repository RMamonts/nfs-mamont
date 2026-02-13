//! `MOUNT` protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.0>.
use crate::vfs::file;
use num_derive::{FromPrimitive, ToPrimitive};

pub mod dump;
pub mod export;
pub mod mnt;
pub mod null;
pub mod umnt;
pub mod umntall;

/// Maximum bytes in a path name.
pub const MOUNT_DIRPATH_LEN: usize = 1024;
/// Maximum bytes in a name.
pub const MOUNT_HOST_NAME_LEN: usize = 255;

pub const MOUNT_PROGRAM: u32 = 100005;
pub const MOUNT_VERSION: u32 = 3;

/// Client host name.
pub type HostName = String;

/// Entry of the list maintained on the server of clients
/// that have requested file handles with the MNT procedure.
#[derive(Clone)]
pub struct MountEntry {
    /// Name of the client host that is sending RPC.
    pub hostname: file::FileName,
    /// Server pathname of a directory.
    pub directory: file::FilePath,
}

/// Status of result of `mount` procedure, that represents correct result
pub const MNT_OK: usize = 0;

/// Possible MOUNT errors
///
/// Used only in [`mnt::Mnt`] because other procedures say
///
/// There are no MOUNT protocol errors which can be returned
/// from this procedure. However, RPC errors may be returned
/// for authentication or other RPC failures.
#[derive(ToPrimitive, FromPrimitive)]
pub enum MntError {
    /// Not owner
    Perm = 1,
    /// No such file or directory
    NoEnt = 2,
    /// I/O error
    Io = 5,
    /// Permission denied
    Access = 13,
    /// Not a directory
    NoDir = 20,
    /// Invalid argument
    Inval = 22,
    /// Filename too long
    NameTooLong = 63,
    /// Operation not supported
    NotSupp = 10004,
    /// A failure on the server
    ServerFault = 10006,
}

/// Export entry, containing list of clients, allowed to
/// mount the specified directory.
#[derive(Clone)]
pub struct ExportEntry {
    /// Exported directory.
    pub directory: file::FilePath,
    /// Client host names. They are implementation specific
    /// and cannot be directly interpreted by clients.
    pub names: Vec<HostName>,
}

/// MOUNT v3 procedures trait.
pub trait Mount:
    null::Null + mnt::Mnt + dump::Dump + umnt::Umnt + umntall::Umntall + export::Export
{
}
