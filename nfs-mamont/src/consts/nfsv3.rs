pub const NFS_PROGRAM: u32 = 100003;
pub const NFS_VERSION: u32 = 3;

pub const NULL: u32 = 0;
pub const GETATTR: u32 = 1;
pub const SETATTR: u32 = 2;
pub const LOOKUP: u32 = 3;
pub const ACCESS: u32 = 4;
pub const READLINK: u32 = 5;
pub const READ: u32 = 6;
pub const WRITE: u32 = 7;
pub const CREATE: u32 = 8;
pub const MKDIR: u32 = 9;
pub const SYMLINK: u32 = 10;
pub const MKNOD: u32 = 11;
pub const REMOVE: u32 = 12;
pub const RMDIR: u32 = 13;
pub const RENAME: u32 = 14;
pub const LINK: u32 = 15;
pub const READDIR: u32 = 16;
pub const READDIRPLUS: u32 = 17;
pub const FSSTAT: u32 = 18;
pub const FSINFO: u32 = 19;
pub const PATHCONF: u32 = 20;
pub const COMMIT: u32 = 21;

pub const NFS3_FHSIZE: usize = 8;

pub const NFS3_COOKIEVERFSIZE: usize = 8;

pub const NFS3_CREATEVERFSIZE: usize = 8;

pub const NFS3_WRITEVERFSIZE: usize = 8;
