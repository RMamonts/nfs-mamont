//! Defines NFSv3 [`Link`] interface.

use async_trait::async_trait;

use crate::vfs::{self};

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
    fn keep(promise: Result);
}

#[async_trait]
pub trait Link {
    /// Creates a hard link from `file` to `name``, in the directory, `dir`.
    ///
    /// # Parameters:
    ///
    /// * `file` --- The file handle for the existing file system object.
    /// * `dir` --- The file handle for the directory in which the link is to be created.
    /// * `name` --- The name that is to be associated with the created link.
    ///
    /// Changes to any property of the hard-linked files are reflected in all of the linked files.
    ///
    /// `file` and `dir` must reside on the same file system and server, means
    /// that the fsid fields in the attributes for the directories are the same. If they reside on different file systems,
    /// the error, [`vfs::Error::XDev`], is returned.
    ///
    /// On some servers, the filenames, "." and "..", are illegal as either from.name or to.name.
    /// In addition, neither from.name nor to.name can be an alias for from.dir. These servers will
    /// return the error, [`vfs::Error::InvalidArgument`], in these cases.
    async fn link(
        &self,
        file: file::Handle,
        dir: file::Handle,
        name: String,
        promise: impl Promise,
    );
}
