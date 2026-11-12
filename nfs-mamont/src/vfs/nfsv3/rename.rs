//! Defines NFSv3 [`Rename`] interface.

use crate::rpc::auth::Credential;
use crate::vfs::nfsv3;

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the directory, `from_dir`.
    pub from_dir_wcc: nfsv3::WccData,
    /// Weak cache consistency data for the directory, `to_dir`.
    pub to_dir_wcc: nfsv3::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: nfsv3::Error,
    /// Weak cache consistency data for the directory, `from_dir`.
    pub from_dir_wcc: nfsv3::WccData,
    /// Weak cache consistency data for the directory, `to_dir`.
    pub to_dir_wcc: nfsv3::WccData,
}

/// [`Rename::rename`] arguments.
pub struct Args {
    /// A [`nfsv3::DirOpArgs`] structure identifying the source (the file
    /// system object to be re-named)
    pub from: nfsv3::DirOpArgs,
    /// A [`nfsv3::DirOpArgs`] structure identifying the target (the new
    /// name of the object):
    pub to: nfsv3::DirOpArgs,
}

#[trait_variant::make(Send)]
pub trait Rename {
    /// Renames the file in the directory.
    ///
    /// The operation is required to be atomic to the client.
    ///
    /// [`Args::to`] and [`Args::from`] must reside on the same file system and server,
    /// means that the fsid fields in the attributes for the directories are the same. If they
    /// reside on different file systems, the error, [`nfsv3::Error::XDev`], is returned.
    ///
    /// Even though the operation is atomic, the status, [`nfsv3::Error::TooManyLinks`], may be
    /// returned if the server used a `"unlink/link/unlink"` sequence internally.
    ///
    /// A file handle may or may not become stale on a rename. However, server implementors are
    /// strongly encouraged to attempt to keep file handles from becoming stale in this fashion.
    ///
    /// On some servers, the filenames, "." and "..", are illegal as either from.name or to.name.
    /// In addition, neither [`Args::from`] nor [`Args::to`]  names can be an alias for
    /// [`Args::from`] dir. These servers will return the error, [`nfsv3::Error::InvalidArgument`],
    /// in these cases.
    ///
    /// If the directory, [`Args::to`] dir, already contains an entry with the name,
    /// [`Args::to`] name, the source object must be compatible with the target: either both are
    /// non-directories or both are directories and the target must be empty. If compatible, the
    /// existing target is removed before the rename occurs. If they are not compatible or if the
    /// target is a directory but not empty, the server should return the error,
    /// [`nfsv3::Error::Exist`].
    ///
    /// If arguments pairs refer to the same file (they might be hard links of each other), then
    /// [`Rename::rename`] should perform no action and return [`Success`].
    async fn rename(&self, args: Args, cred: &Credential) -> Result<Success, Fail>;
}
