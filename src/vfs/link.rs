//! Defines NFSv3 [`Link`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// The post-operation attributes of the file system object identified by `file`.
    pub file_attr: Option<file::Attr>,
    /// Weak cache consistency data for the directory, `link_dir`.
    pub dir_wcc: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// The post-operation attributes of the file system object identified by `file`.
    pub file_attr: Option<file::Attr>,
    /// Weak cache consistency data for the directory, `dir`.
    pub dir_wcc: vfs::WccData,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Link::link`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Link::link`] arguments.
pub struct Args {
    /// The file handle for the existing file system object.
    pub file: file::Handle,
    /// The file handle for the directory in which the link is to be created.
    pub dir: file::Handle,
    /// The name that is to be associated with the created link.
    pub name: file::Name,
}

#[async_trait]
pub trait Link {
    /// Creates a hard link from [`Args::file`] to [`Args::name`], in the directory, [`Args::dir`].
    ///
    /// Changes to any property of the hard-linked files are reflected in all of the linked files.
    ///
    /// [`Args::file`] and [`Args::dir`] must reside on the same file system and server, means
    /// that the fsid fields in the attributes for the directories are the same. If they reside on different file systems,
    /// the error, [`vfs::Error::XDev`], is returned.
    ///
    /// On some servers, the filenames, "." and "..", are illegal as either from.name or to.name. // TODO(i.erin) strange comment.
    /// In addition, neither from.name nor to.name can be an alias for from.dir. These servers will
    /// return the error, [`vfs::Error::InvalidArgument`], in these cases.
    async fn link(&self, args: Args, promise: impl Promise);
}
