//! Defines NFSv3 [`Create`] interface.

use async_trait::async_trait;
use std::path::Path;

use crate::consts::nfsv3::NFS3_CREATEVERFSIZE;
use crate::vfs;

use super::file;

/// Opaque byte array of `NFS3_CREATEVERFSIZE` size used in `How`
pub struct Verifier(pub [u8; NFS3_CREATEVERFSIZE]);

/// Describes how the server is to handle the file creation.
pub enum How {
    /// Means that the file should be created without checking
    /// for the existence of a duplicate file in the same
    /// directory with initial attributes for the file.
    Unchecked(super::set_attr::NewAttr),
    /// Specifies that the server should check for the presence
    /// of a duplicate file before performing the create and
    /// should fail the request with [`vfs::Error::Exist`] if a
    /// duplicate file exists.
    ///
    /// If the file does not exist, the request is performed as
    /// described for [`How::Unchecked`].
    Guarded(super::set_attr::NewAttr),
    /// Specifies that the server is to follow exclusive creation
    /// semantics, using the verifier to ensure exclusive creation
    /// of the target. No attributes provided in this case, since the
    /// server may use the target file metadata to store the [`Verifier`].
    Exclusive(Verifier),
}

/// Describes codes of [`How`] branches as integers.
pub enum HowMode {
    Unchecked = 0,
    Guarded = 1,
    Exclusive = 2,
}

/// Success result.
pub struct Success {
    /// The file handle of the newly created regular file.
    pub file: Option<file::Handle>,
    /// The attributes of the regular file just created.
    pub attr: Option<file::Attr>,
    /// Weak cache consistency data for the directory of creation.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// [`Create::create`] arguments.
pub struct Args {
    /// The location of the file to be created.
    pub object: vfs::DirOpArgs,
    /// The file creation mode. See [`How`] documentation.
    pub how: How,
}

#[async_trait]
pub trait Create {
    /// Creates a [`file::Type::Regular`] file.
    async fn create(&self, args: Args, path: &Path) -> Result<Success, Fail>;
}
