//! Defines NFSv3 [`ReadDirPlus`] interface.

use async_trait::async_trait;

use crate::vfs;
use crate::vfs::read_dir::Cookie;
use crate::vfs::read_dir::CookieVerifier;

use super::file;

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
    /// `true` if the last member of [`Self::entries`] is the last
    /// entry in the directory or the list [`Self::entries`] is
    /// empty and the cookie corresponded to the end of the
    /// directory.
    ///
    /// If `false`, there may be more entries to read.
    pub eof: bool,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// The attributes of the directory, `dir`.
    pub dir_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`ReadDirPlus::read_dir_plus`] result into.
#[async_trait]
pub trait Promise: Send {
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
    /// cookie_verifier must match that returned by the [`ReadDirPlus::read_dir_plus`] in which the cookie
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
