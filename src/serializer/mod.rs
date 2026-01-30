mod mount;
mod nfs;
mod serialize_struct;
#[cfg(test)]
mod tests;

use std::io::{self, Error, ErrorKind, Write};

use byteorder::{BigEndian, WriteBytesExt};
use num_traits::ToPrimitive;

pub const ALIGNMENT: usize = 4;

fn padding(dest: &mut dyn Write, n: usize) -> io::Result<()> {
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    let slice = [0u8; ALIGNMENT];
    dest.write_all(&slice[..padding])
}

pub fn u32(dest: &mut dyn Write, n: u32) -> io::Result<()> {
    dest.write_u32::<BigEndian>(n)
}

pub fn u64(dest: &mut dyn Write, n: u64) -> io::Result<()> {
    dest.write_u64::<BigEndian>(n)
}

pub fn bool(dest: &mut dyn Write, b: bool) -> io::Result<()> {
    match b {
        true => dest.write_u32::<BigEndian>(1),
        false => dest.write_u32::<BigEndian>(0),
    }
}

pub fn option<T>(
    dest: &mut dyn Write,
    opt: Option<T>,
    cont: impl FnOnce(T, &mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    match opt {
        Some(val) => bool(dest, true).and_then(|_| cont(val, dest)),
        None => bool(dest, false),
    }
}

pub fn array<const N: usize>(dest: &mut dyn Write, slice: [u8; N]) -> io::Result<()> {
    dest.write_all(&slice).and_then(|_| padding(dest, N))
}

pub fn vector(dest: &mut dyn Write, vec: Vec<u8>) -> io::Result<()> {
    dest.write_u32::<BigEndian>(vec.len() as u32)
        .and_then(|_| dest.write_all(&vec))
        .and_then(|_| padding(dest, vec.len()))
}

pub fn vec_max_size(dest: &mut dyn Write, vec: Vec<u8>, max_size: usize) -> io::Result<()> {
    if vec.len() > max_size {
        return Err(Error::new(ErrorKind::InvalidInput, "vector out of bounds"));
    }
    vector(dest, vec)
}

pub fn string_max_size(dest: &mut dyn Write, string: String, max_size: usize) -> io::Result<()> {
    vec_max_size(dest, string.into_bytes(), max_size)
}

#[allow(dead_code)]
pub fn string(dest: &mut dyn Write, string: String) -> io::Result<()> {
    vector(dest, string.into_bytes())
}

#[allow(dead_code)]
pub fn variant<T: ToPrimitive>(dest: &mut dyn Write, val: T) -> io::Result<()> {
    dest.write_u32::<BigEndian>(
        ToPrimitive::to_u32(&val)
            .ok_or(Error::new(ErrorKind::InvalidInput, "cannot convert to u32"))?,
    )
}

#[allow(dead_code)]
pub fn usize_as_u32(dest: &mut dyn Write, n: usize) -> io::Result<()> {
    dest.write_u32::<BigEndian>(
        n.to_u32().ok_or(Error::new(ErrorKind::InvalidInput, "cannot convert to u32"))?,
    )
}
