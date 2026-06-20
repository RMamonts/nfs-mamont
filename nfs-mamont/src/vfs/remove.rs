//! Defines NFSv3 [`Remove`] interface.

use nfs_mamont_derive::XDRSize;

use crate::vfs;

/// Success result.
#[derive(XDRSize)]
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
#[derive(XDRSize)]
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory, [`Args::object`] dir.
    pub dir_wcc: vfs::WccData,
}

/// [`Remove::remove`] arguments.
pub struct Args {
    /// A [`vfs::DirOpArgs`] structure identifying the entry to be removed.
    pub object: vfs::DirOpArgs,
}

#[trait_variant::make(Send)]
pub trait Remove {
    /// Removes (deletes) an entry from a directory.
    async fn remove(&self, args: Args) -> Result<Success, Fail>;
}
