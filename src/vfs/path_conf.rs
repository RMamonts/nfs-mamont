//! Defines NFSv3 [`PathConf`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

/// Success result.
pub struct Success {
    /// The attributes of the object specified by `file`.
    pub file_attr: Option<file::Attr>,
    /// The maximum number of hard links to an `file`.
    pub link_max: u32,
    /// The maximum length of a component of a filename.
    pub name_max: u32,
    /// If `true`, the server will reject any request that
    /// includes a name longer than [`Self::name_max`] with the error,
    /// [`vfs::Error::NameTooLong`].
    ///
    /// If `false`, any length name over [`Self::name_max`] bytes will
    /// be silently truncated to [`Self::name_max`] bytes.
    pub no_trunc: bool,
    /// If `true`, the server will reject any request to change
    /// either the owner or the group associated with a file if
    /// the caller is not the privileged user. (Uid 0.)
    pub chown_restricted: bool,
    /// If `true`, the server file system does not distinguish case when interpreting filenames.
    pub case_insensitive: bool,
    /// If `true`, the server file system will preserve the case of a name during a
    /// [`vfs::create::Create::create`], [`vfs::mk_dir::MkDir::mk_dir`],
    /// [`vfs::mk_node::MkNode::mk_node`], [`vfs::symlink::Symlink::symlink`],
    /// [`vfs::rename::Rename::rename`], or [`vfs::link::Link::link`] operation.
    pub case_preserving: bool,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// The attributes of the object specified by `file`.
    pub file_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`FsInfo::fs_info`] result into.
#[async_trait]
pub trait Promise {
    fn keep(promise: Result);
}

#[async_trait]
pub trait PathConf {
    /// Retrieves the pathconf information for a file or directory.
    ///
    /// # Parameters:
    ///
    /// * `file` --- The file handle for the file system object.
    async fn path_conf(&self, file: file::Handle);
}
