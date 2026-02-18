//! Defines NFSv3 [`Commit`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the file.
    pub file_wcc: vfs::WccData,
    /// This is a cookie that the client can use to determine
    /// whether the server has rebooted between a call to [vfs::write::Write::write]
    /// and a subsequent call to [`Commit::commit`].
    pub verifier: vfs::write::Verifier,
}

/// Fail result.
pub struct Fail {
    /// Error on fauler.
    pub error: vfs::Error,
    /// Weak cache consistency data for the file.
    pub file_wcc: vfs::WccData,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Commit::commit`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Commit::commit`] arguments.
pub struct Args {
    /// The file handle for the file to which data is to be flushed.
    pub file: file::Handle,
    /// The position within the file at which the flush is to begin.
    pub offset: u64,
    /// The number of bytes of data to flush. If count is `0`, a
    /// flush from offset to the end of file is done.
    pub count: u32,
}

#[async_trait]
pub trait Commit {
    /// Forces or flushes data to stable storage that was previously written.
    async fn commit(&self, args: Args, promise: impl Promise);
}
