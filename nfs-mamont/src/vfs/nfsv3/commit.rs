//! Defines NFSv3 [`Commit`] interface.
use crate::rpc::auth::Credential;
use crate::vfs::nfsv3;

use super::file;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the file.
    pub file_wcc: nfsv3::WccData,
    /// This is a cookie that the client can use to determine
    /// whether the server has rebooted between a call to [nfsv3::write::Write::write]
    /// and a subsequent call to [`Commit::commit`].
    pub verifier: nfsv3::write::Verifier,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: nfsv3::Error,
    /// Weak cache consistency data for the file.
    pub file_wcc: nfsv3::WccData,
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

#[trait_variant::make(Send)]
pub trait Commit {
    /// Forces or flushes data to stable storage that was previously written.
    async fn commit(&self, args: Args, cred: &Credential) -> Result<Success, Fail>;
}
