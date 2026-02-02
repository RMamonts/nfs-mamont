//! Defines NFSv3 [`Remove`] interface.

use async_trait::async_trait;

use crate::vfs::{self, Error};

use super::file;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    pub status: Error,
    /// Weak cache consistency data for the directory, where.dir.
    /// TODO(use Args structure).
    pub dir_wcc: vfs::WccData,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Remove::remove`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Remove::remove`] arguments.
pub struct Args {
    /// The file handle for the directory from which the entry is to be removed.
    pub dir: file::Handle,
    /// The name of the entry to be removed.
    pub name: String,
}

#[async_trait]
pub trait Remove {
    /// Removes (deletes) an entry from a directory.
    async fn remove(&self, args: Args, promise: impl Promise);
}
