//! Defines the NFSv4 READDIR operation types, RFC 7530 §16.24.

use crate::vfs::nfs_shared::read_dir::Cookie;

use super::file;

/// READDIR arguments (`READDIR4args`).
pub struct Args {
    /// Opaque cookie describing the last directory entry returned.
    pub cookie: Cookie,
    /// Verifier returned by the previous READDIR, used to detect changes.
    pub cookieverf: super::Verifier,
    /// Preferred size of the directory entry portion of the reply.
    pub dircount: u32,
    /// Maximum size of the READDIR4resok reply in bytes.
    pub maxcount: u32,
    /// Bitmask of attributes the client is requesting for each entry.
    pub attr_request: file::Bitmap,
}

/// A directory entry (`entry4`).
pub struct Entry {
    /// Opaque cookie identifying this entry for a subsequent READDIR.
    pub cookie: Cookie,
    /// Name of the entry.
    pub name: file::Name,
    /// Attributes of the entry.
    pub attrs: file::Attr,
}

/// A list of directory entries (`dirlist4`).
pub struct DirList {
    /// Directory entries, in the order read.
    pub entries: Vec<Entry>,
    /// Whether the last entry has been read and `entries` is complete.
    pub eof: bool,
}

/// Success result (`READDIR4resok`).
pub struct Success {
    /// Verifier identifying the state of the directory.
    pub cookieverf: super::Verifier,
    /// The directory entries read.
    pub reply: DirList,
}

/// Fail result (`READDIR4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
