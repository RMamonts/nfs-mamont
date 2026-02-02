//! Defines NFSv3 [`Write`] interface.

use super::{file, Error};
use crate::allocator::Slice;
use crate::vfs;
use async_trait::async_trait;
use num_derive::{FromPrimitive, ToPrimitive};

#[derive(Clone, Copy, Eq, PartialEq, FromPrimitive, ToPrimitive, Debug)]
pub enum StableHow {
    Unstable = 0,
    DataSync = 1,
    FileSync = 2,
}

pub const VERIFIER_LEN: usize = 8;

pub struct Verifier(pub [u8; VERIFIER_LEN]);

/// Success result.
pub struct Success {
    /// Weak cache consistency data for the file.
    pub file_wcc: vfs::WccData,
    /// The number of bytes of data written to the file.
    pub count: u64,
    /// The indication of the level of commitment of the data and metadata.
    pub commited: StableHow,
    /// TODO(what is it?)
    pub verifier: Verifier,
}

/// Fail result.
pub struct Fail {
    pub status: Error,
    /// Weak cache consistency data for the file.
    pub wcc_data: vfs::WccData,
}

pub type Result = std::result::Result<Success, Fail>;

/// Defines callback to pass [`Read::read`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(promise: Result);
}

/// [`Write::write`] arguments.
pub struct Args {
    /// The file handle for the file to which data is to be written.
    /// This must identify a file system object of type [`file::Type::Regular`].
    pub file: file::Handle,
    /// The position within file at which the write is to begin.
    pub offset: u64,
    /// Size of data in `Slice`
    pub size: u32,
    /// If `stable` is [`StableHow::FileSync`], the server must commit the data
    /// written plus all file system metadata to stable storage before returning results.
    /// If `stable` is [`StableHow::DataSync`], then server must commit all of the data
    /// to stable storage and enough of the metadata to retrieve the data before returning.
    /// If `stable` is [`StableHow::Unstable`], the server is free to commit any part of the
    /// `data` and the metadata to stable storage, including all or none, before returning a reply
    /// the client. There is no guarantee whether or when any uncommitted data will subsequently be
    /// commited to stable storage. // TODO(i.erin) move comment to StableHow definition
    pub stable: StableHow,
    /// The data to be written to the file.
    ///
    /// The size of data must be less than or equal to the value of the TODO(wtmax) field.
    /// If greater, the server may write only TODO(wtmax) bytes, resulting in a short write.
    ///
    pub data: Slice,
}

/// equal to `Args` structure used to separate parsing of `Slice` from other fields
pub struct ArgsPartial {
    /// The file handle for the file to which data is to be written.
    /// This must identify a file system object of type [`file::Type::Regular`].
    pub file: file::Handle,
    /// The position within file at which the write is to begin.
    pub offset: u64,
    /// Size of data in `Slice`
    pub size: u32,
    /// If `stable` is [`StableHow::FileSync`], the server must commit the data
    /// written plus all file system metadata to stable storage before returning results.
    /// If `stable` is [`StableHow::DataSync`], then server must commit all of the data
    /// to stable storage and enough of the metadata to retrieve the data before returning.
    /// If `stable` is [`StableHow::Unstable`], the server is free to commit any part of the
    /// `data` and the metadata to stable storage, including all or none, before returning a reply
    /// the client. There is no guarantee whether or when any uncommitted data will subsequently be
    /// commited to stable storage. // TODO(i.erin) move comment to StableHow definition
    pub stable: StableHow,
}

#[async_trait]
pub trait Write {
    /// Writes data to a file.
    ///
    /// Some implementations may return [`vfs::Error::NoSpace`] instead of
    /// [`vfs::Error::QuotaExceeded`] when a user's quota is exceeded.
    ///
    /// If the `file` system object type was not a [`file::Type::Regular`] file,
    /// [`vfs::Error::InvalidArgument`] is returned.
    async fn write(&self, args: Args, promise: impl Promise);
}
