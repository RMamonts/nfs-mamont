//! Defines NFSv3 [`Link`] interface.

use async_trait::async_trait;

use crate::interface::vfs;

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

/// [`Link::link`] arguments.
pub struct Args {
    /// The file handle for the existing file system object.
    pub file: file::Handle,
    /// The location of the link to be created
    pub link: vfs::DirOpArgs,
}

#[async_trait]
pub trait Link {
    /// Creates a hard link from [`Args::file`] to [`Args::link`], in the directory.
    ///
    /// Changes to any property of the hard-linked files are reflected in all of the linked files.
    ///
    /// [`Args::file`] and [`Args::link`] must reside on the same file system and server, means
    /// that the fsid fields in the attributes for the directories are the same. If they reside on different file systems,
    /// the error, [`vfs::Error::XDev`], is returned.
    ///
    /// On some servers, the filenames, "." and "..", are illegal for link names.
    /// In addition, the link name cannot be an alias for the target directory. These servers will
    /// return the error, [`vfs::Error::InvalidArgument`], in these cases.
    async fn link(&self, args: Args) -> Result<Success, Fail>;
}
