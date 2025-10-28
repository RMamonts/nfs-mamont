
pub const UID_SIZE: usize = 8;

/// Unique file identifier.
///
/// Corresponds to the file handle from RFC 1813.
#[derive(Clone)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub struct UID(pub [u8; UID_SIZE]);

/// File type.
#[derive(Clone, Copy)]
pub enum Type {
    Regular,
    Directory,
    BlockDevice,
    CharacterDevice,
    Symlink,
    Socket,
    Fifo,
}

/// File attributes.
#[derive(Clone)]
pub struct Attr {
    pub file_type: Type,
    pub mode: u32,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub used: u64,
    pub device: Option<Device>,
    pub fsid: u64,
    pub fileid: u64,
    pub atime: Time,
    pub mtime: Time,
    pub ctime: Time,
}

/// Time of file [`super::Vfs`] operations.
#[derive(Copy, Clone)]
pub struct Time {
    pub seconds: i64,
    pub nanos: u32,
}

/// Major and minor device pair.
#[derive(Copy, Clone)]
pub struct Device {
    pub major: u32,
    pub minor: u32,
}
