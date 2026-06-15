/// Maximum bytes in a path name.
pub const MOUNT_DIRPATH_LEN: usize = 1024;
/// Maximum bytes in a name.
pub const MOUNT_HOST_NAME_LEN: usize = 255;

pub const MOUNT_PROGRAM: u32 = 100005;
pub const MOUNT_VERSION: u32 = 3;

pub const MOUNT_NULL: u32 = 0;
pub const MOUNT_MNT: u32 = 1;
pub const MOUNT_DUMP: u32 = 2;
pub const MOUNT_UMNT: u32 = 3;
pub const MOUNT_UMNTALL: u32 = 4;
pub const MOUNT_EXPORT: u32 = 5;
