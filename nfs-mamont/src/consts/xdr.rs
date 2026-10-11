//! Constants defined by the XDR (External Data Representation) standard,
//! [RFC 4506](https://datatracker.ietf.org/doc/html/rfc4506).

/// The XDR alignment in bytes.
///
/// Every XDR item is serialized in a multiple of [`ALIGNMENT`] bytes, padded
/// with zero bytes when its length is not already a multiple of it.
/// ([RFC 4506, section 3](https://datatracker.ietf.org/doc/html/rfc4506#section-3))
pub const ALIGNMENT: usize = 4;
