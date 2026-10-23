use num_derive::{FromPrimitive, ToPrimitive};

/// Maximum bytes in a path name.
pub const MOUNT_DIRPATH_LEN: usize = 1024;
/// Maximum bytes in a name.
pub const MOUNT_HOST_NAME_LEN: usize = 255;

pub const MOUNT_PROGRAM: u32 = 100005;
pub const MOUNT_VERSION: u32 = 3;

/// MOUNT protocol procedures (RFC 1813 §5.2) as a type-safe enum.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum MountProc {
    Null = 0,
    Mnt = 1,
    Dump = 2,
    Umnt = 3,
    UmntAll = 4,
    Export = 5,
}
