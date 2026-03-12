//! Defines NFSv3 [`Remove`] interface.

use async_trait::async_trait;

use crate::vfs;

/// Success result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory, [`Args::object`] dir.
    pub dir_wcc: vfs::WccData,
}

/// [`Remove::remove`] arguments.
#[derive(Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, PartialEq, Clone))]
pub struct Args {
    /// A [`vfs::DirOpArgs`] structure identifying the entry to be removed.
    pub object: vfs::DirOpArgs,
}

#[async_trait]
pub trait Remove {
    /// Removes (deletes) an entry from a directory.
    async fn remove(&self, args: Args) -> Result<Success, Fail>;
}
