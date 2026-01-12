//! `MOUNT` protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.0>.

#![allow(dead_code)]

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
/// Maximum bytes in a NFS v3 file handle.
pub const HANDLE_SIZE: usize = 8;

/// Unique file identifier.
///
/// Corresponds to the file handle from RFC 1813.
#[derive(Clone)]
pub struct Handle(pub [u8; HANDLE_SIZE]);

/// Server pathname of a directory.
pub type DirPath = String;

/// Client host name.
pub type HostName = String;

#[derive(Debug)]
pub enum Error {
    /// Not owner
    Permission = 1,
    /// No such file or directory
    NoEntry = 2,
    /// I/O error
    IO = 3,
    /// Permission denied
    Access = 4,
    /// Not a directory
    NotDir = 5,
    /// Invalid argument
    InvalidArgument = 6,
    /// Filename too long
    NameTooLong = 7,
    /// Operation is not supported
    NotSupported = 8,
    /// A failure on the server
    ServerFault = 9,
}

/// Entry of the list maintained on the server of clients
/// that have requested file handles with the MNT procedure.
#[derive(Clone)]
pub struct MountEntry {
    /// Name of the client host that is sending RPC.
    pub hostname: HostName,
    /// Server pathname of a directory.
    pub directory: DirPath,
}

/// Export entry, containing list of clients, allowed to
/// mount the specified directory.
#[derive(Clone)]
pub struct ExportEntry {
    /// Exported directory.
    pub directory: DirPath,
    /// Client host names. They are implementation specific
    /// and cannot be directly interpreted by clients.
    pub name: Vec<HostName>,
}

pub trait Mount {}
