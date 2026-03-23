//! `MOUNT` protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.0>.
pub mod dump;
pub mod export;
pub mod mnt;
pub mod umnt;
pub mod umntall;

use crate::vfs::file;

/// Client host name.
pub type HostName = String;

/// Entry of the list maintained on the server of clients
/// that have requested file handles with the MNT procedure.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MountEntry {
    /// Name of the client host that is sending RPC.
    pub hostname: HostName,
    /// Server pathname of a directory.
    pub directory: file::Path,
}

/// Export entry, containing list of clients, allowed to
/// mount the specified directory.
#[derive(Clone)]
pub struct ExportEntry {
    /// Exported directory.
    pub directory: file::Path,
    /// Client host names. They are implementation specific
    /// and cannot be directly interpreted by clients.
    pub names: Vec<HostName>,
}

/// Wrapper for mount procedure result bodies.
pub enum MountRes {
    Null,
    Mount(Result<mnt::Success, mnt::Fail>),
    Unmount,
    Export(export::Success),
    Dump(dump::Success),
    UnmountAll,
}

// TODO: Remove mount trait
#[allow(dead_code)]
pub trait Mount: mnt::Mnt + umnt::Umnt + umntall::Umntall + export::Export + dump::Dump {}

impl<T> Mount for T where T: mnt::Mnt + umnt::Umnt + umntall::Umntall + export::Export + dump::Dump {}
