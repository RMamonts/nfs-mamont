//! Defines Mount version 3 [`Umnt`] interface (Procedure 3).
//!
//! as defined in RFC 1813 section 5.2.3.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3>.

use async_trait::async_trait;

use crate::vfs::file;

/// Defines callback to pass [`Umnt::umnt`] result into.
#[async_trait]
pub trait Promise {
    async fn keep();
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
    async fn umnt(&self, dirpath: file::FilePath, promise: impl Promise);
}
