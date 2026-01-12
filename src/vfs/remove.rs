//! Defines NFSv3 [`Remove`] interface.

use async_trait::async_trait;

use crate::vfs::{self};

use super::file;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory, where.dir.
    /// TODO(use Args structure).
    pub dir_wcc: vfs::WccData,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Remove::remove`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

#[async_trait]
pub trait Remove {
    /// Removes (deletes) an entry from a directory.
    ///
    /// # Parameters:
    ///
    /// * `dir` --- The file handle for the directory from which the entry is to be removed.
    /// * `name` --- The name of the entry to be removed.
    async fn remove(&self, dir: file::Handle, name: String, promise: impl Promise);
}
