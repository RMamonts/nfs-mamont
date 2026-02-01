//! Defines NFSv3 [`SetAttr`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

pub type Result = std::result::Result<Success, Fail>;

/// Guard used by [`SetAttr::set_attr`].
#[derive(Copy, Clone)]
pub struct Guard {
    pub ctime: file::Time,
}

/// Defines callback to pass [`SetAttr::set_attr`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// Strategy for updating timestamps in [`SetAttr`].
#[derive(Copy, Clone)]
pub enum SetTime {
    DontChange,
    ToServer,
    ToClient(file::Time),
}

/// Specifies which attribute to update via [`SetAttr::set_attr`].
pub struct NewAttr {
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub size: Option<u64>,
    pub atime: SetTime,
    pub mtime: SetTime,
}

/// [`SetAttr::set_attr`] arguments.
pub struct Args {
    /// The file handle for the object.
    pub file: file::Handle,
    /// Structure describing the attributes to be set and the new values for those attributes.
    pub new_attr: NewAttr,
    /// Optionally verify that `ctime` of the object matches the client expectation.
    pub guard: Option<Guard>,
}

pub struct Success {
    pub wcc_data: vfs::WccData,
}

pub struct Fail {
    pub wcc_data: vfs::WccData,
}

#[async_trait]
pub trait SetAttr {
    /// Changes one or more of the attributes of a file system object on the server.
    ///
    /// If [`Args::guard`] is [`Some`] and object ctime differs from the guard one then
    /// implementation must preserve the object attributes and must return a status of
    /// [`vfs::Error::NotSync`].
    ///
    /// [`SetAttr::set_attr`] is not guaranteed atomic. A failed [`SetAttr::set_attr`]
    /// may partially change a file's attributes.
    ///
    /// The [`Args::new_attr`] size field is used to request changes to the size of a file.
    /// A value of `0` causes the file to be truncated, a value less then the current size
    /// of the file causes data from new size to the end off the file to be discarded,
    /// and a size greater than the current size of the file causes logically zeroed
    /// data bytes to be added to the end of the file. Implementation are free to
    /// implement this using holes or actual zero data bytes. Implementation must support
    /// extending the file size.
    ///
    /// Changing the size of a file with [`SetAttr::set_attr`] indirectly
    /// changes the `mtime`.
    ///
    /// [`vfs::Error::InvalidArgument`] may be returned
    /// - if implementation can not store a uid or gid in its own representation
    /// - if implementation can only support 32 bit offset and sizes,
    ///   and [`SetAttr::set_attr`] request to set the size of a file to larger than
    ///   can be represented in 32 bit.
    async fn set_attr(&self, args: Args, promise: impl Promise);
}
