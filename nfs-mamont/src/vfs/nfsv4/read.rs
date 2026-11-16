//! Defines the NFSv4 READ operation types, RFC 7530 §16.23.

use crate::vfs::nfsv4::file;
use crate::Buffer;

/// READ arguments (`READ4args`).
pub struct Args {
    /// State id of the file to read from, previously returned by an OPEN,
    /// byte-range lock, or delegation.
    pub stateid: file::StateId,
    /// Byte offset within the file at which to start reading.
    pub offset: u64,
    /// Maximum number of bytes to read.
    pub count: u32,
}

/// Success result (`READ4resok`).
pub struct Success<B: Buffer> {
    /// Whether the read ended at the end of the file.
    pub eof: bool,
    /// The data read, at most `Args::count` bytes.
    pub data: B,
}

/// Fail result (`READ4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
