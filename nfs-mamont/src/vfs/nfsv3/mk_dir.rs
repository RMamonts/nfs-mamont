//! Defines NFSv3 [`MkDir`] interface.

use crate::rpc::auth::Credential;
use crate::vfs::nfsv3;

use super::file;

/// Success result.
pub struct Success {
    /// The file handle for the newly created directory.
    pub file: Option<file::Handle>,
    /// The attributes for the newly created subdirectory.
    pub attr: Option<file::Attr>,
    /// Weak cache consistency data for the [`Args::object`] dir.
    pub wcc_data: nfsv3::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: nfsv3::Error,
    /// Weak cache consistency data for the directory, [`Args::object`] dir.
    pub dir_wcc: nfsv3::WccData,
}

/// [`MkDir::mk_dir`] arguments.
pub struct Args {
    /// The location of the subdirectory to be created.
    pub object: nfsv3::DirOpArgs,
    /// The initial attributes for the subdirectory.
    pub attr: super::set_attr::NewAttr,
}

#[trait_variant::make(Send)]
pub trait MkDir {
    /// Creates a new subdirectory.
    ///
    /// Returns [`nfsv3::Error::Exist`] for "." or ".." `name`.
    async fn mk_dir(&self, args: Args, cred: &Credential) -> Result<Success, Fail>;
}
