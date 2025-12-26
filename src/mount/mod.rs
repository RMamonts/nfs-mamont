//! `MOUNT` protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.0>.

#![allow(dead_code)]

pub mod mnt;

pub const MNTPATHLEN: usize = 1024;
pub const MNTNAMLEN: usize = 255;
pub const FHSIZE3: usize = 64;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct FileHandle(pub Vec<u8>);

impl FileHandle {
    pub fn new(bytes: Vec<u8>) -> Self {
        assert!(bytes.len() <= FHSIZE3);
        Self(bytes)
    }
}

/// Directory path
pub type DirPath = String;

/// Client/host name
pub type Name = String;

#[derive(Debug)]
pub enum Error {
    /// Not owner
    Permission,
    /// No such file or directory
    NoEntry,
    /// I/O error
    IO,
    /// Permission denied
    Access,
    /// Not a directory
    NotDir,
    /// Invalid argument
    InvalidArgument,
    /// Filename too long
    NameTooLong,
    /// Operation is not supported
    NotSupported,
    /// A failure on the server
    ServerFault,
}

#[derive(Clone)]
pub struct MountEntry {
    pub hostname: Name,
    pub directory: DirPath,
}

#[derive(Clone)]
pub struct Group {
    pub name: Name,
}

#[derive(Clone)]
pub struct ExportEntry {
    pub directory: DirPath,
    pub groups: Vec<Group>,
}

pub trait Mount {}
