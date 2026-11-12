//! Defines NFSv3 [`Remove`] interface.

use crate::rpc::auth::Credential;
use crate::vfs::nfsv3;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory.
    pub wcc_data: nfsv3::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: nfsv3::Error,
    /// Weak cache consistency data for the directory, [`Args::object`] dir.
    pub dir_wcc: nfsv3::WccData,
}

/// [`Remove::remove`] arguments.
pub struct Args {
    /// A [`nfsv3::DirOpArgs`] structure identifying the entry to be removed.
    pub object: nfsv3::DirOpArgs,
}

#[trait_variant::make(Send)]
pub trait Remove {
    /// Removes (deletes) an entry from a directory.
    async fn remove(&self, args: Args, cred: &Credential) -> Result<Success, Fail>;
}
