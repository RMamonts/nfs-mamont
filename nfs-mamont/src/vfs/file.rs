use std::io;
use std::path::PathBuf;

use num_derive::{FromPrimitive, ToPrimitive};

use crate::vfs::{MAX_NAME_LEN, MAX_PATH_LEN};

use crate::consts::nfsv3::NFS3_FHSIZE;

/// Index of a backend registered in [`crate::BackendRegistry`].
///
/// Encoded in the first byte of every [`Handle`], which limits the server to
/// [`BackendId::MAX`] + 1 simultaneously attached backends.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendId(u8);

impl BackendId {
    /// The lowest backend index that can be assigned.
    pub const MIN: Self = Self(u8::MIN);
    /// The highest backend index that can be assigned.
    pub const MAX: Self = Self(u8::MAX);

    /// Builds a backend id from its raw index.
    #[inline]
    pub fn new(id: u8) -> BackendId {
        BackendId(id)
    }

    /// Returns the raw index of the backend id.
    #[inline]
    pub fn raw(&self) -> u8 {
        self.0
    }
}

/// Size of the backend-specific part of a [`Handle`], in bytes.
pub const PAYLOAD_SIZE: usize = NFS3_FHSIZE - 1;

/// Unique file identifier.
///
/// Corresponds to the file handle from RFC 1813.
///
/// The first byte is the [`BackendId`] of the backend that owns the object; the
/// remaining [`PAYLOAD_SIZE`] bytes are opaque to the server and interpreted only
/// by that backend.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle([u8; NFS3_FHSIZE]);

impl Handle {
    /// Builds a handle owned by `backend` from a backend-specific `payload`.
    pub fn new(backend: BackendId, payload: [u8; PAYLOAD_SIZE]) -> Self {
        let mut raw = [0u8; NFS3_FHSIZE];
        raw[0] = backend.raw();
        raw[1..].copy_from_slice(&payload);
        Handle(raw)
    }

    /// Returns the index of the backend that owns the object behind this handle.
    #[inline]
    pub fn backend_id(&self) -> BackendId {
        BackendId(self.0[0])
    }

    /// Returns the backend-specific part of the handle.
    #[inline]
    pub fn payload(&self) -> [u8; PAYLOAD_SIZE] {
        let mut payload = [0u8; PAYLOAD_SIZE];
        payload.copy_from_slice(&self.0[1..]);
        payload
    }

    /// Returns the raw bytes of the handle.
    #[cfg(test)]
    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8; NFS3_FHSIZE] {
        &self.0
    }

    /// Consumes the handle and returns its raw bytes.
    #[inline]
    pub(crate) fn into_bytes(self) -> [u8; NFS3_FHSIZE] {
        self.0
    }

    /// Builds a handle from raw bytes as received from the NFS client.
    #[inline]
    pub(crate) fn from_bytes(bytes: [u8; NFS3_FHSIZE]) -> Self {
        Handle(bytes)
    }
}

/// A validated wrapper around a `String` representing a name.
///
/// [`Name`] ensures that the inner string does not exceed [`MAX_NAME_LEN`].
/// It provides safe construction, accessors, and conversion back into the
/// owned inner [`String`].
#[derive(Debug, PartialEq, Clone)]
pub struct Name(String);

impl Name {
    /// Creates a new [`Name`] after validating its length.
    ///
    /// # Errors
    ///
    /// Returns an error if `name.len() > MAX_NAME_LEN`.
    pub fn new(name: String) -> io::Result<Self> {
        if name.len() > MAX_NAME_LEN {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name too long"));
        }
        if name.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name is empty"));
        }
        if name.contains('/') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "name contains path separator",
            ));
        }
        Ok(Name(name))
    }

    /// Consumes the wrapper and returns the inner [`String`].
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Returns the inner name as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated wrapper around a [`PathBuf`].
///
/// [`Path`] ensures that the provided path string does not exceed
/// [`MAX_PATH_LEN`]. It offers safe construction, accessors, and
/// conversion back into the owned [`PathBuf`].
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Path(PathBuf);

impl Path {
    /// Creates a new [`Path`] from a string after validating its length.
    ///
    /// # Errors
    ///
    /// Returns an error if `path.len() > MAX_PATH_LEN`.
    pub fn new(path: String) -> io::Result<Self> {
        if path.len() > MAX_PATH_LEN {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "path too long"));
        }
        if path.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "path is empty"));
        }
        Ok(Path(PathBuf::from(path)))
    }

    /// Consumes the wrapper and returns the inner [`PathBuf`].
    #[inline]
    pub fn into_inner(self) -> PathBuf {
        self.0
    }

    /// Returns the inner path as a `&Path`.
    #[inline]
    pub fn as_path(&self) -> &std::path::Path {
        self.0.as_path()
    }
}

/// Type of file.
#[repr(u32)]
#[derive(Clone, Copy, FromPrimitive, ToPrimitive)]
pub enum Type {
    /// Regular file.
    Regular = 1,
    /// Directory.
    Directory = 2,
    /// Block special device file.
    BlockDevice = 3,
    /// Character special device file.
    CharacterDevice = 4,
    /// Symbolic link.
    Symlink = 5,
    /// Socket file.
    Socket = 6,
    /// Named pipe.
    Fifo = 7,
}

/// File attributes, also known as `fattr3` in RFC
#[derive(Clone)]
pub struct Attr {
    /// Type of the file, see [`Type`].
    pub file_type: Type,
    /// Protection mode bits.
    pub mode: u32,
    /// Number of hard links to the file.
    pub nlink: u32,
    /// User ID of the owner of the file.
    pub uid: u32,
    /// Group ID of the group of the file.
    pub gid: u32,
    /// Size of the file of bytes.
    pub size: u64,
    /// The number of bytes of disk space that the file actually uses.
    pub used: u64,
    /// Describes the device file if the file type is [`Type::BlockDevice`]
    /// or [`Type::CharacterDevice`].
    ///
    /// See [`Type`].
    pub device: Device,
    /// The file system identifier for the file system.
    pub fs_id: u64,
    /// The number which uniquely identifies the file within its file system.
    pub file_id: u64,
    /// The time when the file data was last accessed.
    pub atime: Time,
    /// The time when the file data was last modified.
    pub mtime: Time,
    /// The time when the attributes of the file were last changed.
    ///
    /// Writing to the file changes the ctime in addition to the mtime.
    pub ctime: Time,
}

/// Time of file [`super::Vfs`] operations.
///
/// Gives the number of seconds and nanoseconds since midnight January 1, 1970 Greenwich Mean Time.
/// It is used to pass time and date information. The times associated with files are all server
/// times except in the case of a [`super::set_attr`] operation where the client can
/// explicitly set the file time.
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Copy, Clone)]
pub struct Time {
    pub seconds: u32,
    pub nanos: u32,
}

/// Major and minor device pair.
///
/// Used only for [`Type::BlockDevice`] and [`Type::CharacterDevice`] file types.
#[derive(Copy, Clone)]
pub struct Device {
    pub major: u32,
    pub minor: u32,
}

/// Weak cache consistency attributes.
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Copy, Clone)]
pub struct WccAttr {
    /// The file size in bytes of the object before the operation.
    pub size: u64,
    /// The time of last modification of the object before the operation.
    pub mtime: Time,
    /// The time of last change to the attributes of the object before the operation.
    pub ctime: Time,
}
