//! Constants defined by the NFS protocol version 3, [RFC 1813](https://datatracker.ietf.org/doc/html/rfc1813).

/// RPC program number of the NFS service.
/// ([RFC 1813, section 2.2](https://datatracker.ietf.org/doc/html/rfc1813#section-2.2))
pub const NFS_PROGRAM: u32 = 100003;
/// Version of the NFS protocol implemented by the server.
/// ([RFC 1813, section 2.2](https://datatracker.ietf.org/doc/html/rfc1813#section-2.2))
pub const NFS_VERSION: u32 = 3;

/// Size of a file handle in bytes.
///
/// The first byte carries the [`crate::BackendId`] of the backend owning the object,
/// the rest identifies the object inside that backend. The value is not a multiple of
/// the XDR alignment; padding is added by the serializer and skipped by the parser.
///
/// The protocol only bounds the handle size by 64 bytes
/// ([RFC 1813, section 2.4](https://datatracker.ietf.org/doc/html/rfc1813#section-2.4)).
pub const NFS3_FHSIZE: usize = 9;
/// Size in bytes of the cookie verifier passed to `READDIR` and `READDIRPLUS`.
/// ([RFC 1813, section 2.4](https://datatracker.ietf.org/doc/html/rfc1813#section-2.4))
pub const NFS3_COOKIEVERFSIZE: usize = 8;
/// Size in bytes of the verifier used by exclusive `CREATE`.
/// ([RFC 1813, section 2.4](https://datatracker.ietf.org/doc/html/rfc1813#section-2.4))
pub const NFS3_CREATEVERFSIZE: usize = 8;
/// Size in bytes of the write verifier returned by `WRITE` and `COMMIT`.
/// ([RFC 1813, section 2.4](https://datatracker.ietf.org/doc/html/rfc1813#section-2.4))
pub const NFS3_WRITEVERFSIZE: usize = 8;

/// `NFSPROC3_NULL`: do nothing, used to check that the server responds.
/// ([RFC 1813, section 3.3.0](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.0))
pub const NULL: u32 = 0;
/// `NFSPROC3_GETATTR`: get file attributes.
/// ([RFC 1813, section 3.3.1](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.1))
pub const GETATTR: u32 = 1;
/// `NFSPROC3_SETATTR`: set file attributes.
/// ([RFC 1813, section 3.3.2](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.2))
pub const SETATTR: u32 = 2;
/// `NFSPROC3_LOOKUP`: look up a file name in a directory.
/// ([RFC 1813, section 3.3.3](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.3))
pub const LOOKUP: u32 = 3;
/// `NFSPROC3_ACCESS`: check access permission.
/// ([RFC 1813, section 3.3.4](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.4))
pub const ACCESS: u32 = 4;
/// `NFSPROC3_READLINK`: read the data of a symbolic link.
/// ([RFC 1813, section 3.3.5](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.5))
pub const READLINK: u32 = 5;
/// `NFSPROC3_READ`: read from a file.
/// ([RFC 1813, section 3.3.6](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.6))
pub const READ: u32 = 6;
/// `NFSPROC3_WRITE`: write to a file.
/// ([RFC 1813, section 3.3.7](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.7))
pub const WRITE: u32 = 7;
/// `NFSPROC3_CREATE`: create a regular file.
/// ([RFC 1813, section 3.3.8](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.8))
pub const CREATE: u32 = 8;
/// `NFSPROC3_MKDIR`: create a directory.
/// ([RFC 1813, section 3.3.9](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.9))
pub const MKDIR: u32 = 9;
/// `NFSPROC3_SYMLINK`: create a symbolic link.
/// ([RFC 1813, section 3.3.10](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.10))
pub const SYMLINK: u32 = 10;
/// `NFSPROC3_MKNOD`: create a special device.
/// ([RFC 1813, section 3.3.11](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.11))
pub const MKNOD: u32 = 11;
/// `NFSPROC3_REMOVE`: remove a file.
/// ([RFC 1813, section 3.3.12](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.12))
pub const REMOVE: u32 = 12;
/// `NFSPROC3_RMDIR`: remove a directory.
/// ([RFC 1813, section 3.3.13](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.13))
pub const RMDIR: u32 = 13;
/// `NFSPROC3_RENAME`: rename a file or directory.
/// ([RFC 1813, section 3.3.14](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.14))
pub const RENAME: u32 = 14;
/// `NFSPROC3_LINK`: create a hard link to an object.
/// ([RFC 1813, section 3.3.15](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.15))
pub const LINK: u32 = 15;
/// `NFSPROC3_READDIR`: read entries from a directory.
/// ([RFC 1813, section 3.3.16](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.16))
pub const READDIR: u32 = 16;
/// `NFSPROC3_READDIRPLUS`: read entries from a directory together with their attributes and handles.
/// ([RFC 1813, section 3.3.17](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.17))
pub const READDIRPLUS: u32 = 17;
/// `NFSPROC3_FSSTAT`: get dynamic file system information.
/// ([RFC 1813, section 3.3.18](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.18))
pub const FSSTAT: u32 = 18;
/// `NFSPROC3_FSINFO`: get static file system information.
/// ([RFC 1813, section 3.3.19](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.19))
pub const FSINFO: u32 = 19;
/// `NFSPROC3_PATHCONF`: retrieve POSIX information.
/// ([RFC 1813, section 3.3.20](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.20))
pub const PATHCONF: u32 = 20;
/// `NFSPROC3_COMMIT`: commit cached data on the server to stable storage.
/// ([RFC 1813, section 3.3.21](https://datatracker.ietf.org/doc/html/rfc1813#section-3.3.21))
pub const COMMIT: u32 = 21;
