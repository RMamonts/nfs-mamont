use std::io::Read;
use std::mem::MaybeUninit;

use byteorder::{BigEndian, ReadBytesExt};
use num_traits::{FromPrimitive, ToPrimitive};

use crate::rpc::Error;

use super::Result;

#[allow(dead_code)]
pub const ALIGNMENT: usize = 4;

#[allow(dead_code)]
pub fn padding(src: &mut dyn Read, n: usize) -> Result<()> {
    let mut buf = [0u8; ALIGNMENT];
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    src.read_exact(&mut buf[..padding]).map_err(|_| Error::IncorrectPadding)
}

#[allow(dead_code)]
pub fn u8(src: &mut dyn Read) -> Result<u8> {
    src.read_u8().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn u32(src: &mut dyn Read) -> Result<u32> {
    src.read_u32::<BigEndian>().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn u64(src: &mut dyn Read) -> Result<u64> {
    src.read_u64::<BigEndian>().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn bool(src: &mut dyn Read) -> Result<bool> {
    match u32(src)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn option<T>(
    src: &mut dyn Read,
    cont: impl FnOnce(&mut dyn Read) -> Result<T>,
) -> Result<Option<T>> {
    match bool(src)? {
        true => Ok(Some(cont(src)?)),
        false => Ok(None),
    }
}

#[allow(dead_code)]
pub fn array<const N: usize, T: Copy>(
    src: &mut dyn Read,
    cont: impl Fn(&mut dyn Read) -> Result<T>,
) -> Result<[T; N]> {
    let mut buf: [MaybeUninit<T>; N] = [const { MaybeUninit::uninit() }; N];
    for elem in buf.iter_mut() {
        elem.write(cont(src)?);
    }
    padding(src, N * size_of::<T>())?;
    Ok(unsafe { buf.as_ptr().cast::<[T; N]>().read() })
}

#[allow(dead_code)]
pub fn vector<T>(src: &mut dyn Read, cont: impl Fn(&mut dyn Read) -> Result<T>) -> Result<Vec<T>> {
    let size = u32_as_usize(src)?;
    let mut vec = Vec::with_capacity(size);
    for _ in 0..size {
        vec.push(cont(src)?);
    }
    padding(src, size_of::<T>() * size)?;
    Ok(vec)
}

#[allow(dead_code)]
pub fn vec_max_size<T>(
    src: &mut dyn Read,
    cont: impl Fn(&mut dyn Read) -> Result<T>,
    max_size: usize,
) -> Result<Vec<T>> {
    let size = u32_as_usize(src)?;
    if size > max_size {
        return Err(Error::MaxELemLimit);
    }
    let mut vec = Vec::with_capacity(size);
    for _ in 0..size {
        vec.push(cont(src)?);
    }
    padding(src, size_of::<T>() * size)?;
    Ok(vec)
}

#[allow(dead_code)]
pub fn string_max_size(src: &mut dyn Read, max_size: usize) -> Result<String> {
    let vec = vec_max_size(src, |s| u8(s), max_size)?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

#[allow(dead_code)]
pub fn string(src: &mut dyn Read) -> Result<String> {
    let vec = vector(src, |s| u8(s))?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

#[allow(dead_code)]
pub fn variant<T: FromPrimitive>(src: &mut dyn Read) -> Result<T> {
    FromPrimitive::from_u32(u32(src)?).ok_or(Error::EnumDiscMismatch)
}

#[allow(dead_code)]
pub fn u32_as_usize(src: &mut dyn Read) -> Result<usize> {
    u32(src)?.to_usize().ok_or(Error::ImpossibleTypeCast)
}
