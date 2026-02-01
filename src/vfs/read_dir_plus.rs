//! Defines NFSv3 [`ReadDirPlus`] interface.

use async_trait::async_trait;

use crate::vfs::read_dir::Cookie;
use crate::vfs::read_dir::CookieVerifier;

use super::file;

// also keep in mind, that it should have some pointer to next item in list
pub struct Entry {
    /// Since UNIX clients give a special meaning to the fileid
    /// value zero, UNIX clients should be careful to map zero
    /// fileid values to some other value and servers should try
    /// to avoid sending a zero fileid.
    pub file_id: u64,
    pub file_name: String,
    pub cookie: Cookie,
    pub file_attr: Option<file::Attr>,
    pub file_handle: Option<file::Handle>,
}

/// Success result.
pub struct Success {
    /// The attributes of the directory, `dir`.
    pub dir_attr: Option<file::Attr>,
    /// The cookie verifier.
    pub cookie_verifier: CookieVerifier,
    /// Zero or more directory [`Entry`] entries.
    pub entries: Vec<Entry>,
}

/// Fail result.
pub struct Fail {
    /// The attributes of the directory, `dir`.
    pub dir_attr: Option<file::Attr>,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`ReadDir::read_dir`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`ReadDirPlus::read_dir_plus`] arguments
pub struct Args {
    /// The file handle for the directory to be read.
    pub dir: file::Handle,
    /// This should be set to 0 on the first request to read a directory.
    /// On subsequent requests, it should be a cookie as returned by the server.
    pub cookie: Cookie,
    /// This should be set to 0 in the first request to read the directory.
    /// On subsequent requests, it should be a cookie_verifier as returned by the server. The
    /// cookie_verifier must match that returned by the [`ReadDir::read_dir`] in which the cookie
    /// was acquired.
    pub cookie_verifier: CookieVerifier,
    /// The maximum number of bytes of directory information returned. This number should not
    /// include the size of the attributes and file handle portions of the result.
    pub dir_count: u32,
    /// The maximum size of the [`Success`] structure, in bytes. The size must include all XDR
    /// overhead. The server is free to return fewer than maxcount bytes of data.
    pub max_count: u32,
}

#[async_trait]
pub trait ReadDirPlus {
    /// Retrieves a variable number of entries from a file system directory and returns complete
    /// information about each.
    async fn read_dir_plus(&self, args: Args, promise: impl Promise);
}
