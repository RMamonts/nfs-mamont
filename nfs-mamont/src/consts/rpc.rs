//! Constants defined by RPC protocol, RFC 5531

pub const MAX_AUTH_SIZE: usize = 400;

/// Maximum length of the `machinename` field in `AUTH_SYS` credentials (RFC 5531, appendix A).
pub const AUTH_SYS_MAX_MACHINE_NAME: usize = 255;

/// Maximum number of auxiliary GIDs in `AUTH_SYS` credentials (RFC 5531, appendix A).
pub const AUTH_SYS_MAX_GIDS: usize = 16;

/// Max size of RMS fragment data
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
pub const MAX_FRAGMENT_SIZE: usize = 0x7FFF_FFFF;

/// Header mask of RMS
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
pub const HEADER_MASK: usize = 0x8000_0000;

/// RMS frame header size in bytes.
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
pub const RMS_HEADER_SIZE: usize = 4;

/// Remote Procedure Call Protocol Version 2
pub const RPC_VERSION: u32 = 2;
