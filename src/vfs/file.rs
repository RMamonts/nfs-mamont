pub const UID_SIZE: usize = 8;

/// Unique file identifier.
///
/// Corresponds to the file handle from RFC 1813.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Uid(pub [u8; UID_SIZE]);

/// Type of file.
#[derive(Clone, Copy)]
pub enum Type {
    /// Regular file.
    Regular = 1,
    /// Directory.
    Directory = 2,
    /// Block special device file.
    BlockDevice= 3,
    /// Character special device file.
    CharacterDevice = 4,
    /// Symbolik link.
    Symlink = 5,
    /// Socket file.
    Socket = 6,
    /// Named pipe.
    Fifo = 7,
}

/// File attributes.
#[derive(Clone)]
pub struct Attr {
    /// Type of the file, see [`Type`].
    pub file_type: Type,
    /// Protection mode bits.
    pub mode: u32,
    /// Number of hard links to the file.
    pub nlink: u32,
    /// User ID of the owner of the file.
    pub uid: u32,
    /// Group ID of the group of the file.
    pub gid: u32,
    /// Size of the file of bytes.
    pub size: u64,
    /// The number of bytes of disk space that the file actually uses.
    pub used: u64,
    /// Describes the device file if the file type is [`Type::BlockDevice`]
    /// or [`Type::CharacterDevice`].
    ///
    /// See [`Type`].
    pub device: Option<Device>,
    /// The file system identifier for the file system.
    pub fsid: u64,
    /// The number which uniquely identifies the file within its file system.
    pub fileid: u64,
    /// The time when the file data was last accessed.
    pub atime: Time,
    /// The time when the file data was last modified.
    pub mtime: Time,
    /// The time when the attributes of the file were last changed.
    ///
    /// Writing to the file changes the ctime in addition to the mtime.
    pub ctime: Time,
}

/// Time of file [`super::Vfs`] operations.
///
/// Gives the number of seconds and nanoseconds since midnight January 1, 1970 Greenwich Mean Time.
/// It is used to pass time and date information. The times associated with files are all server
/// times except in the case of a [`super::Vfs::set_attr`] operation where the client can
/// explicitly set the file time.
#[derive(Copy, Clone)]
pub struct Time {
    pub seconds: u32,
    pub nanos: u32,
}

/// Major and minor device pair.
///
/// Used only for [`Type::BlockDevice`] and [`Type::CharacterDevice`] file types.
#[derive(Copy, Clone)]
pub struct Device {
    pub major: u32,
    pub minor: u32,
}

/// Weak cache consistency attributes.
#[derive(Copy, Clone)]
pub struct WccAttr {
    /// The file size in bytes of the object before the operation.
    pub size: u64,
    /// The time of last modification of the object before the operation.
    pub mtime: Time,
    /// The time of last change to the attributes of the object before the operation.
    pub ctime: Time,
}

