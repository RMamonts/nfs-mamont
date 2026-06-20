//! `MOUNT` protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.0>.
pub mod dump;
pub mod export;
pub mod mnt;
pub mod umnt;
pub mod umntall;

use std::io;

use nfs_mamont_derive::XDRSize;

use crate::consts::mount::MOUNT_HOST_NAME_LEN;
use crate::vfs::file;
use crate::xdr;
/// Client host name.
#[derive(Clone, Debug, PartialEq, Hash, Eq, XDRSize)]
pub struct HostName(String);

impl HostName {
    pub fn new(name: String) -> io::Result<Self> {
        if name.len() > MOUNT_HOST_NAME_LEN {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "host name too long"));
        }
        Ok(HostName(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Entry of the list maintained on the server of clients
/// that have requested file handles with the MNT procedure.
#[derive(Clone, Debug, PartialEq, Eq, Hash, XDRSize)]
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

impl xdr::XDRSize for ExportEntry {
    fn xdr_size(&self) -> usize {
        self.directory.xdr_size()
            + Self::INTEGER
            + self.names.iter().map(|name| name.xdr_size() + Self::INTEGER).sum::<usize>()
    }
}

/// Wrapper for mount procedure result bodies.
#[derive(XDRSize)]
pub enum MountRes {
    Null,
    Mount(Result<mnt::Success, mnt::Fail>),
    Unmount,
    Export(export::Success),
    Dump(dump::Success),
    UnmountAll,
}

#[allow(dead_code)]
pub trait Mount: mnt::Mnt + umnt::Umnt + umntall::Umntall + export::Export + dump::Dump {}

impl<T> Mount for T where T: mnt::Mnt + umnt::Umnt + umntall::Umntall + export::Export + dump::Dump {}
