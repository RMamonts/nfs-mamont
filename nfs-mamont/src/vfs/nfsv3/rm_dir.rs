//! Defines NFSv3 [`RmDir`] interface.

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
    /// Weak cache consistency data for the directory from [`Args::object`].
    pub dir_wcc: nfsv3::WccData,
}

/// [`RmDir::rm_dir`] arguments.
pub struct Args {
    /// A [`nfsv3::DirOpArgs`] structure identifying the directory entry
    /// to be removed.
    pub object: nfsv3::DirOpArgs,
}

#[trait_variant::make(Send)]
pub trait RmDir {
    /// Removes (deletes) a subdirectory from a directory.
    ///
    /// On some servers, the filename, ".", is illegal. These servers will return
    /// the error, [`nfsv3::Error::InvalidArgument`].
    ///
    /// On some servers, the filename, "..", is illegal. These servers will return
    /// the error, [`nfsv3::Error::Exist`].
    async fn rm_dir(&self, args: Args, cred: &Credential) -> Result<Success, Fail>;
}
