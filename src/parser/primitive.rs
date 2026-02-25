//! Primitive XDR data type parsing utilities.

use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;

use super::{Error, Result};
use crate::vfs::MAX_PATH_LEN;
use byteorder::{BigEndian, ReadBytesExt};
use num_traits::{FromPrimitive, ToPrimitive};

/// The XDR alignment in bytes.
#[allow(dead_code)]
pub const ALIGNMENT: usize = 4;

/// Reads and discards padding bytes to ensure XDR alignment.
#[allow(dead_code)]
pub fn padding(src: &mut impl Read, n: usize) -> Result<()> {
    let mut buf = [0u8; ALIGNMENT];
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    src.read_exact(&mut buf[..padding]).map_err(|_| Error::IncorrectPadding)
}

/// Parses a `u8` (byte) from the `Read` source.
#[allow(dead_code)]
pub fn u8(src: &mut impl Read) -> Result<u8> {
    src.read_u8().map_err(Error::IO)
}

/// Parses a `u32` (unsigned 32-bit integer) from the `Read` source, in Big-Endian format.
#[allow(dead_code)]
pub fn u32(src: &mut impl Read) -> Result<u32> {
    src.read_u32::<BigEndian>().map_err(Error::IO)
}

/// Parses a `u64` (unsigned 64-bit integer) from the `Read` source, in Big-Endian format.
#[allow(dead_code)]
pub fn u64(src: &mut impl Read) -> Result<u64> {
    src.read_u64::<BigEndian>().map_err(Error::IO)
}

/// Parses an XDR boolean (encoded as a `u32`) from the `Read` source.
#[allow(dead_code)]
pub fn bool(src: &mut impl Read) -> Result<bool> {
    match u32(src)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Parses an optional XDR type. The option is encoded as a boolean preceding the actual type.
#[allow(dead_code)]
pub fn option<T, S: Read>(
    src: &mut S,
    cont: impl FnOnce(&mut S) -> Result<T>,
) -> Result<Option<T>> {
    match bool(src)? {
        true => Ok(Some(cont(src)?)),
        false => Ok(None),
    }
}

/// Parses a fixed-size array of bytes `[u8; N]` from the `Read` source, including padding.
#[allow(dead_code)]
pub fn array<const N: usize>(src: &mut impl Read) -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    src.read_exact(&mut buf).map_err(Error::IO)?;
    padding(src, N)?;
    Ok(buf)
}

/// Parses a variable-length vector of bytes (opaque data) from the `Read` source.
/// The vector's length is encoded as a `u32` preceding the data.
#[allow(dead_code)]
pub fn vector(src: &mut impl Read) -> Result<Vec<u8>> {
    let size = u32_as_usize(src)?;
    let mut vec = vec![0u8; size];
    src.read_exact(vec.as_mut_slice()).map_err(Error::IO)?;
    padding(src, size)?;
    Ok(vec)
}

/// Parses a variable-length vector of bytes with a maximum allowed size.
#[allow(dead_code)]
pub fn vec_max_size(src: &mut impl Read, max_size: usize) -> Result<Vec<u8>> {
    let size = u32_as_usize(src)?;
    if size > max_size {
        return Err(Error::MaxElemLimit);
    }
    let mut vec = vec![0u8; size];
    src.read_exact(vec.as_mut_slice()).map_err(Error::IO)?;
    padding(src, size)?;
    Ok(vec)
}

/// Parses an XDR string with a maximum allowed size.
#[allow(dead_code)]
pub fn string_max_size(src: &mut impl Read, max_size: usize) -> Result<String> {
    let vec = vec_max_size(src, max_size)?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

/// Parses an XDR string from the `Read` source.
#[allow(dead_code)]
pub fn string(src: &mut impl Read) -> Result<String> {
    let vec = vector(src)?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

/// Parses an XDR-encoded path from the `Read` source.
pub fn path(src: &mut impl Read) -> Result<PathBuf> {
    PathBuf::from_str(string_max_size(src, MAX_PATH_LEN)?.as_str()).map_err(|_| Error::MaxElemLimit)
}

/// Parses an XDR enum variant from the `Read` source.
#[allow(dead_code)]
pub fn variant<T: FromPrimitive>(src: &mut impl Read) -> Result<T> {
    FromPrimitive::from_u32(u32(src)?).ok_or(Error::EnumDiscMismatch)
}

/// Parses a `u32` from the `Read` source and converts it to `usize`.
#[allow(dead_code)]
pub fn u32_as_usize(src: &mut impl Read) -> Result<usize> {
    u32(src)?.to_usize().ok_or(Error::ImpossibleTypeCast)
}
