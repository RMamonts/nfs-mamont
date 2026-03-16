//! Defines Mount version 3 [`Umnt`] interface (Procedure 3).
//!
//! as defined in RFC 1813 section 5.2.3.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3>.

use async_trait::async_trait;

use crate::vfs::file;

/// Arguments for the Unmount operation, containing the path to be unmounted.
<<<<<<< HEAD
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Clone))]
pub struct UnmountArgs(pub file::Path);
=======
#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct Args {
    pub dirpath: file::Path,
}

#[async_trait]
pub trait Umnt {
    /// Removes the mount list entry for the directory that was
    /// previously the subject of a MNT call from this client.
    ///
    /// # Parameters:
    /// * `dirpath` --- a server pathname of a directory.
    ///
    /// AUTH_UNIX authentication or better is required.
    /// There are no MOUNT protocol errors which can be returned from this procedure.
    async fn umnt(&self, args: Args);
}
>>>>>>> svmk17/fix_auth_parsing
