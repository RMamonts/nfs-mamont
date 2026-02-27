//! Defines NFSv3 [`FsInfo`] interface.

use async_trait::async_trait;

use crate::vfs;

use super::file;

pub struct Properties(u32);

impl Properties {
    pub const LINK: u32 = 0x0001;
    pub const SYMLINK: u32 = 0x0002;
    pub const HOMOGENEOUS: u32 = 0x0008;
    pub const CANSETTIME: u32 = 0x0010;

    pub const ALL: u32 = Self::LINK | Self::SYMLINK | Self::HOMOGENEOUS | Self::CANSETTIME;

    pub fn from_wire(raw: u32) -> Self {
        Self(raw & Self::ALL)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

/// Success result.
pub struct Success {
    /// The attributes of the file system root.
    pub root_attr: Option<file::Attr>,
    /// The maximum size in bytes of a [`vfs::read::Read::read`] request supported by the server.
    /// Any [`vfs::read::Read::read`] with a number greater than rtmax will result in a short
    /// read of rtmax bytes or less.
    pub read_max: u32,
    /// The preferred size of a [`vfs::read::Read::read`] request. This should be the same as
    /// rtmax unless there is a clear benefit in performance or efficiency.
    pub read_pref: u32,
    /// The suggested multiple for the size of a [`vfs::read::Read::read`] request.
    pub read_mult: u32,
    /// The maximum size of a [`vfs::write::Write::write`] request supported by the server.
    /// In general, the client is limited by [`Self::write_max`] since there is no guarantee that a
    /// server can handle a larger write. Any [`vfs::write::Write::write`] with a count greater
    /// than [`Self::write_max`] will result in a short write of at most [`Self::write_max`] bytes.
    pub write_max: u32,
    /// The preferred size of a [`vfs::write::Write::write`] request. This should be
    /// the same as [`Self::write_max`] unless there is a clear benefit in performance or efficiency.
    pub write_pref: u32,
    /// The suggested multiple for the size of a [`vfs::write::Write::write`] request.
    pub write_mult: u32,
    /// The preferred size of a [`vfs::read_dir::ReadDir::read_dir`] request.
    pub read_dir_pref: u32,
    /// The maximum size of a file on the file system.
    pub max_file_size: u64,
    /// The server time granularity.
    ///
    /// When setting a file time using [`vfs::set_attr::SetAttr::set_attr`],
    /// the server guarantees only to preserve times to this accuracy.
    pub time_delta: vfs::file::Time,
    /// A bit mask of file system properties.
    pub properties: Properties,
}

/// Fail result.
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// The attributes of the file system root.
    pub root_attr: Option<file::Attr>,
}

type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`FsInfo::fs_info`] result into.
#[async_trait]
pub trait Promise: Send {
    async fn keep(promise: Result);
}

/// [`FsInfo::fs_info`] arguments.
pub struct Args {
    /// A file handle identifying a mount point in the file system.
    pub root: file::Handle,
}

#[async_trait]
pub trait FsInfo {
    /// Retrieves nonvolatile file system state information and general information.
    async fn fs_info(&self, args: Args, promise: impl Promise);
}
