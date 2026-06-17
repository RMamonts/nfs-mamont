//! Defines NFSv3 [`Write`] interface.
use arbitrary::{Arbitrary, Unstructured};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::ToPrimitive;

use crate::allocator::Buffer;
use crate::consts::nfsv3::NFS3_WRITEVERFSIZE;
use crate::vfs;

use super::file;

/// Enum describing servers behaviour after performing write:
///
/// If `stable` is [`StableHow::FileSync`], the server must commit the data
/// written plus all file system metadata to stable storage before returning results.
/// If `stable` is [`StableHow::DataSync`], then server must commit all of the data
/// to stable storage and enough of the metadata to retrieve the data before returning.
/// If `stable` is [`StableHow::Unstable`], the server is free to commit any part of the
/// `data` and the metadata to stable storage, including all or none, before returning a reply
/// the client. There is no guarantee whether or when any uncommitted data will subsequently be
/// committed to stable storage.
#[derive(Clone, Copy, Eq, PartialEq, FromPrimitive, ToPrimitive, Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum StableHow {
    Unstable = 0,
    DataSync = 1,
    FileSync = 2,
}

/// Opaque byte array of [`NFS3_WRITEVERFSIZE`] used in [`Success`]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Verifier(pub [u8; NFS3_WRITEVERFSIZE]);

/// Success result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Success {
    /// Weak cache consistency data for the file.
    pub file_wcc: vfs::WccData,
    /// The number of bytes of data written to the file.
    pub count: u32,
    /// The indication of the level of commitment of the data and metadata.
    pub committed: StableHow,
    /// Cookie used by client to detect server reboot between unstable writes and [`vfs::commit::Commit`].
    pub verifier: Verifier,
}

/// Fail result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// Weak cache consistency data for the file.
    pub wcc_data: vfs::WccData,
}

/// [`Write::write`] arguments.
#[cfg_attr(feature = "arbitrary", derive(Clone, Debug))]
pub struct Args<B: Buffer> {
    /// The file handle for the file to which data is to be written.
    /// This must identify a file system object of type [`file::Type::Regular`].
    pub file: file::Handle,
    /// The position within file at which the write is to begin.
    pub offset: u64,
    /// Size of data in buffer
    pub size: u32,
    /// Server's behaviour after performing write
    pub stable: StableHow,
    /// The data to be written to the file.
    ///
    /// The size of data must be less than or equal to the value of the server's
    /// [`super::fs_info::Success::write_max`] field. If greater, the server may write fewer bytes,
    /// resulting in a short write.
    pub data: B,
}

#[cfg(feature = "arbitrary")]
impl<'a, B: Buffer + Arbitrary<'a>> arbitrary::Arbitrary<'a> for Args<B> {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let data = B::arbitrary(u)?;
        Ok(Self {
            file: u.arbitrary()?,
            offset: u.arbitrary()?,
            size: data.len().to_u32().ok_or(arbitrary::Error::IncorrectFormat)?,
            stable: u.arbitrary()?,
            data,
        })
    }
}

/// equal to `Args` structure used to separate parsing of buffer from other fields
pub struct ArgsPartial {
    /// The file handle for the file to which data is to be written.
    /// This must identify a file system object of type [`file::Type::Regular`].
    pub file: file::Handle,
    /// The position within file at which the write is to begin.
    pub offset: u64,
    /// Size of data in `Buffer`
    pub size: u32,
    /// If `stable` is [`StableHow::FileSync`], the server must commit the data
    /// written plus all file system metadata to stable storage before returning results.
    /// If `stable` is [`StableHow::DataSync`], then server must commit all of the data
    /// to stable storage and enough of the metadata to retrieve the data before returning.
    /// If `stable` is [`StableHow::Unstable`], the server is free to commit any part of the
    /// `data` and the metadata to stable storage, including all or none, before returning a reply
    /// the client. There is no guarantee whether or when any uncommitted data will subsequently be
    /// committed to stable storage.
    pub stable: StableHow,
}

#[trait_variant::make(Send)]
pub trait Write<B: Buffer> {
    /// Writes data to a file.
    ///
    /// Some implementations may return [`vfs::Error::NoSpace`] instead of
    /// [`vfs::Error::QuotaExceeded`] when a user's quota is exceeded.
    ///
    /// If the `file` system object type was not a [`file::Type::Regular`] file,
    /// [`vfs::Error::InvalidArgument`] is returned.
    async fn write(&self, args: Args<B>) -> Result<Success, Fail>;
}
