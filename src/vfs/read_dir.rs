//! Defines NFSv3 [`ReadDir`] interface.

use async_trait::async_trait;

use super::{file, Error};

pub const COOKIE_VERF_SIZE: usize = 8;

// as in RFC, but probably can change to [u8; 8]
/// Identifies a point in the directory.
pub type Cookie = u64;

// as in RFC
/// Verifies that point identified by [`Cookie`] is still valid.
pub struct CookieVerifier(pub [u8; COOKIE_VERF_SIZE]);

// not exactly as in RFC, but possible
pub struct Entry {
    /// Since UNIX clients give a special meaning to the fileid
    /// value zero, UNIX clients should be careful to map zero
    /// fileid values to some other value and servers should try
    /// to avoid sending a zero fileid.
    pub file_id: u64,
    pub file_name: file::FileName,
    pub cookie: Cookie,
}

/// Success result.
pub struct Success {
    /// The attributes of the directory, `dir`.
    pub dir_attr: Option<file::Attr>,
    /// The cookie verifier.
    pub cookie_verifier: CookieVerifier,
    /// Zero or more directory [`Entry`] entries. Represent linked list of [`Entry`]
    pub entries: Vec<Entry>,
}

/// Fail result.
pub struct Fail {
    pub status: Error,
    /// The attributes of the directory, `dir`.
    pub dir_attr: Option<file::Attr>,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`ReadDir::read_dir`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`ReadDir::read_dir`] arguments.
pub struct Args {
    /// The file handle for the directory to be read.
    pub dir: file::Handle,
    /// This should be set to 0 in the first request to read the directory.
    /// On subsequent requests, it should be a cookie as returned by the server.
    pub cookie: Cookie,
    /// This should be set to 0 in the first request to read the directory.
    /// On subsequent requests, it should be a cookie_verifier as returned by the server. The
    /// cookie_verifier must match that returned by the [`ReadDir::read_dir`] in which the cookie
    /// was acquired.
    pub cookie_verifier: CookieVerifier,
    /// The maximum size of the [`Success`] structure, in bytes. The size must include
    /// all XDR overhead. The server is free to return less than count bytes of data.
    pub count: u32,
}

#[async_trait]
pub trait ReadDir {
    /// Retrieves a variable number of entries, in sequence, from a directory.
    ///
    /// If the server detects that the cookie is no longer valid, the server will reject the
    /// [`ReadDir::read_dir`] request with the status, [`vfs::Error::BadCookie`].
    ///
    /// The server may return fewer than `count`` bytes of XDR-encoded entries.
    /// The `count` specified by the client in the request should be greater than or equal to
    /// TODO(FSINFO dtpref).
    async fn read_dir(&self, args: Args, promise: impl Promise);
}
