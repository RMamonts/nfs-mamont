//! Constants defined by the XDR (External Data Representation) standard, RFC 4506.

/// The XDR alignment in bytes.
///
/// Every XDR item is serialized in a multiple of [`ALIGNMENT`] bytes, padded
/// with zero bytes when its length is not already a multiple of it.
pub const ALIGNMENT: usize = 4;
