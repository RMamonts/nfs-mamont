//! Defines the NFSv4 WRITE operation types, RFC 7530 §16.36.

use crate::vfs::nfsv4::file;
use crate::Buffer;

/// The level of stability requested of the server for a write
/// (`stable_how4`), RFC 7530 §16.36.2.
pub enum StableHow {
    /// UNSTABLE4 - the server is not required to commit the data.
    Unstable,
    /// DATA_SYNC4 - the server must commit the data, but not necessarily
    /// all accompanying file system metadata.
    DataSync,
    /// FILE_SYNC4 - the server must commit the data and all accompanying
    /// file system metadata.
    FileSync,
}

/// WRITE arguments (`WRITE4args`).
pub struct Args<B: Buffer> {
    /// State id of the file to write to, previously returned by an OPEN,
    /// byte-range lock, or delegation.
    pub stateid: file::StateId,
    /// Byte offset within the file at which to start writing.
    pub offset: u64,
    /// The level of stability requested of the data.
    pub stable: StableHow,
    /// The data to write.
    pub data: B,
}

/// Success result (`WRITE4resok`).
pub struct Success {
    /// Number of bytes of data written.
    pub count: u32,
    /// The level of commitment of the written data and metadata.
    pub committed: StableHow,
    /// Write verifier of the server instance.
    pub writeverf: super::Verifier,
}

/// Fail result (`WRITE4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
