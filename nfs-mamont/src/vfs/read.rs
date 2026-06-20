//! Defines NFSv3 [`Read`] interface.

use crate::allocator::Buffer;
use crate::vfs;
use crate::vfs::file;

/// Success result.
#[cfg_attr(feature = "arbitrary", derive(Debug))]
pub struct Success<B: Buffer> {
    /// The attributes of the file on completion of the read.
    pub head: SuccessPartial,
    /// The counted data read from the file.
    pub data: B,
}

#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct SuccessPartial {
    /// The attributes of the file on completion of the read.
    pub file_attr: Option<file::Attr>,
    /// The number of bytes of data returned by the read.
    pub count: u32,
    /// If the read ended at the end-of-file.
    pub eof: bool,
}

/// Fail result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Fail {
    /// Error on failure.
    pub error: vfs::Error,
    /// The post-operation attributes of the file.
    pub file_attr: Option<file::Attr>,
}

/// [`Read::read`] arguments.
#[derive(Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, PartialEq, Clone))]
pub struct Args {
    /// The file handle of the file from which data is to be read.
    /// This must identify a file system object of type [`file::Type::Regular`],
    /// otherwise [`Fail`] with [`vfs::Error::InvalidArgument`] is returned.
    pub file: file::Handle,
    /// The position within file at which the read is to begin. If
    /// `offset` is greater than or equal to the size of the file, the [`Success`] is
    /// returned with `count` set to 0
    pub offset: u64,
    /// The number of bytes of data that are to be read. If count is `0`, the
    /// [`Read::read`] will succeed and return `0` bytes of data. Must be less than or equal
    /// to the value of the server's [`super::fs_info::Success::read_max`] field. If greater,
    /// the server may return fewer bytes, resulting in a short read.
    pub count: u32,
}

#[cfg(feature = "arbitrary")]
impl<'a, B> arbitrary::Arbitrary<'a> for Success<B>
where
    B: arbitrary::Arbitrary<'a> + Buffer,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let data = B::arbitrary(u)?;
        let count = data.len();
        assert!(count < u32::MAX as usize);
        Ok(Self {
            head: SuccessPartial {
                file_attr: u.arbitrary::<Option<file::Attr>>()?,
                count: count as u32,
                eof: u.arbitrary::<bool>()?,
            },
            data,
        })
    }
}

#[trait_variant::make(Send)]
pub trait Read<B: Buffer> {
    /// Reads data from a file into a server-provided buffer.
    ///
    /// The `data` buffer is allocated by NFS-Mamont allocator and must be
    /// filled by implementation. This keeps allocation policy under server control.
    async fn read(&self, args: Args, data: B) -> Result<Success<B>, Fail>;
}
