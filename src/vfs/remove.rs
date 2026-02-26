//! Defines NFSv3 [`Remove`] interface.

use async_trait::async_trait;

use crate::vfs::{self};

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory, [`Args::object`] dir.
    /// TODO(use Args structure).
    pub dir_wcc: vfs::WccData,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Remove::remove`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Remove::remove`] arguments.
pub struct Args {
    /// A [`vfs::DirOpArgs`] structure identifying the entry to be removed.
    pub object: vfs::DirOpArgs,
}

#[async_trait]
pub trait Remove {
    /// Removes (deletes) an entry from a directory.
    async fn remove(&self, args: Args, promise: impl Promise);
}
