//! Defines NFSv3 [`MkDir`] interface.

use crate::vfs;
use nfs_mamont_derive::XDRSize;

use super::file;

/// Success result.
#[derive(XDRSize)]
pub struct Success {
    /// The file handle for the newly created directory.
    pub file: Option<file::Handle>,
    /// The attributes for the newly created subdirectory.
    pub attr: Option<file::Attr>,
    /// Weak cache consistency data for the [`Args::object`] dir.
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

/// [`MkDir::mk_dir`] arguments.
pub struct Args {
    /// The location of the subdirectory to be created.
    pub object: vfs::DirOpArgs,
    /// The initial attributes for the subdirectory.
    pub attr: super::set_attr::NewAttr,
}

#[trait_variant::make(Send)]
pub trait MkDir {
    /// Creates a new subdirectory.
    ///
    /// Returns [`vfs::Error::Exist`] for "." or ".." `name`.
    async fn mk_dir(&self, args: Args) -> Result<Success, Fail>;
}
