//! Constants defined by the RPC protocol, [RFC 5531](https://datatracker.ietf.org/doc/html/rfc5531).

/// Version of the RPC protocol implemented by the server (`rpcvers` in a call message).
/// ([RFC 5531, section 9](https://datatracker.ietf.org/doc/html/rfc5531#section-9))
pub const RPC_VERSION: u32 = 2;

/// Maximum length in bytes of the body of an `opaque_auth` structure.
/// ([RFC 5531, section 8.2](https://datatracker.ietf.org/doc/html/rfc5531#section-8.2))
pub const MAX_AUTH_SIZE: usize = 400;
/// Maximum length of the `machinename` field in `AUTH_SYS` credentials.
/// ([RFC 5531, appendix A](https://datatracker.ietf.org/doc/html/rfc5531#page-25))
pub const AUTH_SYS_MAX_MACHINE_NAME: usize = 255;
/// Maximum number of auxiliary GIDs in `AUTH_SYS` credentials.
/// ([RFC 5531, appendix A](https://datatracker.ietf.org/doc/html/rfc5531#page-25))
pub const AUTH_SYS_MAX_GIDS: usize = 16;

/// Size in bytes of a record marking fragment header.
/// ([RFC 5531, section 11](https://datatracker.ietf.org/doc/html/rfc5531#section-11))
pub const RMS_HEADER_SIZE: usize = 4;
/// Mask of the "last fragment" flag in a record marking header: the highest bit.
/// ([RFC 5531, section 11](https://datatracker.ietf.org/doc/html/rfc5531#section-11))
pub const HEADER_MASK: usize = 0x8000_0000;
/// Mask of the fragment length in a record marking header: the lower 31 bits.
/// ([RFC 5531, section 11](https://datatracker.ietf.org/doc/html/rfc5531#section-11))
pub const MAX_FRAGMENT_SIZE: usize = 0x7FFF_FFFF;
