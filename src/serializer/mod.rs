//! XDR (External Data Representation) serialization for NFS and RPC protocols.
//!
//! This module provides serialization functions that convert Rust data types into
//! XDR format, which is the standard data representation used by NFS (Network
//! File System) and RPC (Remote Procedure Call) protocols. XDR ensures
//! consistent data representation across different architectures by enforcing:
//!
//! - **Big-endian byte order**: All multibyte values are serialized in
//!   big-endian (network byte order)
//! - **4-byte alignment**: All data structures are aligned to 4-byte boundaries
//!   with padding bytes inserted as needed

#![allow(dead_code)]

mod files;
mod mount;
mod nfs;
pub mod rpc;
pub mod serialize_struct;
#[cfg(test)]
mod tests;

use std::io::{self, Error, ErrorKind, Write};

use byteorder::{BigEndian, WriteBytesExt};
use num_traits::ToPrimitive;

/// All serialized data is aligned to [`ALIGNMENT`] (4 bytes) boundaries.
pub const ALIGNMENT: usize = 4;

/// Writes XDR alignment padding for an already-written field of length `n` bytes.
///
/// XDR requires 4-byte alignment; this emits zero bytes until the next multiple of [`ALIGNMENT`].
fn padding(dest: &mut dyn Write, n: usize) -> io::Result<()> {
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    let slice = [0u8; ALIGNMENT];
    dest.write_all(&slice[..padding])
}

/// Serializes an XDR `unsigned int` (32-bit) in big-endian order.
pub fn u32(dest: &mut dyn Write, n: u32) -> io::Result<()> {
    dest.write_u32::<BigEndian>(n)
}

/// Serializes an XDR `unsigned hyper` (64-bit) in big-endian order.
pub fn u64(dest: &mut dyn Write, n: u64) -> io::Result<()> {
    dest.write_u64::<BigEndian>(n)
}

/// Serializes an XDR `bool` as `0`/`1` (encoded as a 32-bit integer).
pub fn bool(dest: &mut dyn Write, b: bool) -> io::Result<()> {
    match b {
        true => dest.write_u32::<BigEndian>(1),
        false => dest.write_u32::<BigEndian>(0),
    }
}

/// Serializes an XDR optional value as a boolean discriminator followed by the value (if present).
pub fn option<T, S: Write>(
    dest: &mut S,
    opt: Option<T>,
    cont: impl FnOnce(T, &mut S) -> io::Result<()>,
) -> io::Result<()> {
    match opt {
        Some(val) => bool(dest, true).and_then(|_| cont(val, dest)),
        None => bool(dest, false),
    }
}

/// Serializes a fixed-length XDR opaque value (`opaque[N]`) and adds alignment padding.
pub fn array<const N: usize>(dest: &mut dyn Write, slice: [u8; N]) -> io::Result<()> {
    dest.write_all(&slice).and_then(|_| padding(dest, N))
}

/// Serializes a variable-length XDR opaque value (`opaque<>`): length + bytes + padding.
pub fn vector(dest: &mut dyn Write, vec: &[u8]) -> io::Result<()> {
    let len = vec
        .len()
        .try_into()
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "vector length exceeds u32"))?;
    dest.write_u32::<BigEndian>(len)
        .and_then(|_| dest.write_all(&vec))
        .and_then(|_| padding(dest, vec.len()))
}

/// Serializes a variable-length XDR opaque value with an explicit maximum length check.
pub fn vec_max_size(dest: &mut dyn Write, vec: &[u8], max_size: usize) -> io::Result<()> {
    if vec.len() > max_size {
        return Err(Error::new(ErrorKind::InvalidInput, "vector out of bounds"));
    }
    vector(dest, vec)
}

/// Serializes an XDR `string<max_size>` (UTF-8 bytes as counted opaque, bounded).
pub fn string_max_size(dest: &mut dyn Write, string: String, max_size: usize) -> io::Result<()> {
    vec_max_size(dest, &string.into_bytes(), max_size)
}

#[allow(dead_code)]
/// Serializes an unbounded XDR `string<>` (UTF-8 bytes as counted opaque).
pub fn string(dest: &mut dyn Write, string: String) -> io::Result<()> {
    vector(dest, &string.into_bytes())
}

/// Serializes an XDR enum discriminant / union tag as a 32-bit integer.
pub fn variant<T: ToPrimitive>(dest: &mut impl Write, val: T) -> io::Result<()> {
    dest.write_u32::<BigEndian>(
        ToPrimitive::to_u32(&val)
            .ok_or(Error::new(ErrorKind::InvalidInput, "cannot convert to u32"))?,
    )
}

/// Serializes a Rust `usize` as an XDR `unsigned int` (32-bit), failing on overflow.
pub fn usize_as_u32(dest: &mut dyn Write, n: usize) -> io::Result<()> {
    dest.write_u32::<BigEndian>(
        n.to_u32().ok_or(Error::new(ErrorKind::InvalidInput, "cannot convert to u32"))?,
    )
}
