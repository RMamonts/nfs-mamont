//! Constants defined by RPC protocol, RFC 5531

/// Max size of RMS fragment data
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
pub const MAX_FRAGMENT_SIZE: usize = 0x7FFF_FFFF;

/// Header mask of RMS
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
pub const HEADER_MASK: usize = 0x8000_0000;

/// RMS frame header size in bytes.
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
pub const RMS_HEADER_SIZE: usize = 4;
