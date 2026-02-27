//! Defines NFSv3 [`MkNode`] interface.

use async_trait::async_trait;

use crate::vfs::{self};

use super::file;
use super::set_attr::NewAttr;

/// A discriminated union identifying the type of the special file to be created.
pub enum What {
    /// Create character special file with specified initial attributes and device numbers.
    Char(NewAttr, file::Device),
    /// Create block special file with specified initial attributes and device numbers.
    Block(NewAttr, file::Device),
    /// Create socket special file with specified initial attributes.
    Socket(NewAttr),
    /// Create fifo special file with specified initial attributes.
    Fifo(NewAttr),
    /// Create regular file with no additional data.
    Regular,
    /// Create directory with no additional data.
    Directory,
    /// Create symbolic link with no additional data.
    SymbolicLink,
}

/// Success result.
pub struct Success {
    /// The file handle for the newly created special file.
    pub file: Option<file::Handle>,
    /// The attributes for the newly created special file.
    pub attr: Option<file::Attr>,
    /// Weak cache consistency data for the directory.
    pub wcc_data: vfs::WccData,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the directory from [`Args::object`].
    pub dir_wcc: vfs::WccData,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`MkNode::mk_node`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`MkNode::mk_node`] arguments.
pub struct Args {
    /// The location of the special file to be created
    pub object: vfs::DirOpArgs,
    /// The type of the special file to be created. See [`What`].
    pub what: What,
}

#[async_trait]
pub trait MkNode {
    /// Creates a new special file of the type `what`.
    ///
    /// If the server does not support any of the defined types, the error,
    /// [`vfs::Error::NotSupported`], should be returned.
    ///
    /// Otherwise, if the server does not support the target type the error,
    /// [`vfs::Error::BadType`], should be returned.
    async fn mk_node(&self, args: Args, promise: impl Promise);
}
