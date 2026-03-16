//! `MOUNT` protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.0>.
#![allow(dead_code)]

use std::io;

use crate::vfs::file;

pub mod dump;
pub mod export;
pub mod mnt;
pub mod umnt;
pub mod umntall;

/// Maximum bytes in a path name.
pub const MOUNT_DIRPATH_LEN: usize = 1024;
/// Maximum bytes in a name.
pub const MOUNT_HOST_NAME_LEN: usize = 255;

pub const MOUNT_PROGRAM: u32 = 100005;
pub const MOUNT_VERSION: u32 = 3;

pub const NULL: u32 = 0;
pub const MOUNT: u32 = 1;
pub const DUMP: u32 = 2;
pub const UNMOUNT: u32 = 3;
pub const UNMOUNTALL: u32 = 4;
pub const EXPORT: u32 = 5;

/// Client host name.
#[derive(Clone, Debug)]
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

#[cfg(feature = "arbitrary")]
impl arbitrary::Arbitrary<'_> for HostName {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let size = u.int_in_range(1..=MOUNT_HOST_NAME_LEN)?;
        let mut bytes = vec![0u8; size];
        u.fill_buffer(&mut bytes)?;
        let s = String::from_utf8_lossy(&bytes).to_string();
        HostName::new(s).map_err(|_| arbitrary::Error::IncorrectFormat)
    }
}

/// Entry of the list maintained on the server of clients
/// that have requested file handles with the MNT procedure.
#[derive(Clone)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct MountEntry {
    /// Name of the client host that is sending RPC.
    pub hostname: HostName,
    /// Server pathname of a directory.
    pub directory: file::Path,
}

/// Export entry, containing list of clients, allowed to
/// mount the specified directory.
#[derive(Clone)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct ExportEntry {
    /// Exported directory.
    pub directory: file::Path,
    /// Client host names. They are implementation specific
    /// and cannot be directly interpreted by clients.
    pub names: Vec<HostName>,
}

/// Wrapper for mount procedure result bodies.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub enum MountRes {
    Null,
    Mount(Result<mnt::Success, mnt::Fail>),
    Unmount,
    Export(export::Success),
    Dump(dump::Success),
    UnmountAll,
}

pub trait Mount: mnt::Mnt + umnt::Umnt + umntall::Umntall + export::Export + dump::Dump {}

impl<T> Mount for T where T: mnt::Mnt + umnt::Umnt + umntall::Umntall + export::Export + dump::Dump {}
