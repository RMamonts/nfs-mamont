//! Constants defined by the MOUNT protocol version 3,
//! [RFC 1813 (appendix I)](https://datatracker.ietf.org/doc/html/rfc1813#section-5.0).

/// RPC program number of the MOUNT service.
/// ([RFC 1813, section 5.1.2](https://datatracker.ietf.org/doc/html/rfc1813#section-5.1.2))
pub const MOUNT_PROGRAM: u32 = 100005;
/// Version of the MOUNT protocol implemented by the server.
/// ([RFC 1813, section 5.1.2](https://datatracker.ietf.org/doc/html/rfc1813#section-5.1.2))
pub const MOUNT_VERSION: u32 = 3;

/// Maximum bytes in a path name (`MNTPATHLEN`).
/// ([RFC 1813, section 5.1.4](https://datatracker.ietf.org/doc/html/rfc1813#section-5.1.4))
pub const MOUNT_DIRPATH_LEN: usize = 1024;
/// Maximum bytes in a name (`MNTNAMLEN`).
/// ([RFC 1813, section 5.1.4](https://datatracker.ietf.org/doc/html/rfc1813#section-5.1.4))
pub const MOUNT_HOST_NAME_LEN: usize = 255;

/// `MOUNTPROC3_NULL`: do nothing, used to check that the server responds.
/// ([RFC 1813, section 5.2.0](https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.0))
pub const MOUNT_NULL: u32 = 0;
/// `MOUNTPROC3_MNT`: map a path name to the file handle of an export.
/// ([RFC 1813, section 5.2.1](https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.1))
pub const MOUNT_MNT: u32 = 1;
/// `MOUNTPROC3_DUMP`: list the remotely mounted file systems.
/// ([RFC 1813, section 5.2.2](https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.2))
pub const MOUNT_DUMP: u32 = 2;
/// `MOUNTPROC3_UMNT`: remove a mount entry for the client.
/// ([RFC 1813, section 5.2.3](https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3))
pub const MOUNT_UMNT: u32 = 3;
/// `MOUNTPROC3_UMNTALL`: remove all mount entries for the client.
/// ([RFC 1813, section 5.2.4](https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.4))
pub const MOUNT_UMNTALL: u32 = 4;
/// `MOUNTPROC3_EXPORT`: list the exported file systems.
/// ([RFC 1813, section 5.2.5](https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.5))
pub const MOUNT_EXPORT: u32 = 5;
