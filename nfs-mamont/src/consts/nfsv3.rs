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

/// Size of a file handle in bytes.
///
/// The first byte carries the [`crate::BackendId`] of the backend owning the object,
/// the rest identifies the object inside that backend. The value is not a multiple of
/// the XDR alignment; padding is added by the serializer and skipped by the parser.
pub const NFS3_FHSIZE: usize = 9;

/// Maximum size of a file handle on the wire (`NFS3_FHSIZE` in RFC 1813).
///
/// A longer handle is malformed XDR. A handle that fits but differs from
/// [`NFS3_FHSIZE`] is well-formed, it just cannot have been issued by this server.
pub const NFS3_MAX_FHSIZE: usize = 64;

pub const NFS3_COOKIEVERFSIZE: usize = 8;

pub const NFS3_CREATEVERFSIZE: usize = 8;

pub const NFS3_WRITEVERFSIZE: usize = 8;

/// Largest `READ` or `WRITE` payload a client sends, in bytes.
///
/// The Linux client caps `rsize`/`wsize` at 1 MiB, so no legitimate call
/// carries more data.
pub const MAX_PAYLOAD_SIZE: usize = 1 << 20;
