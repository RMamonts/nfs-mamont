use std::io::Read;
use std::mem::MaybeUninit;

use byteorder::{BigEndian, ReadBytesExt};
use num_traits::{FromPrimitive, ToPrimitive};

use super::Error;

#[allow(dead_code)]
pub const ALIGNMENT: usize = 4;

#[allow(dead_code)]
fn read_padding(src: &mut dyn Read, n: usize) -> Result<(), Error> {
    let mut buf = [0u8; ALIGNMENT];
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    src.read_exact(&mut buf[..padding]).map_err(|_| Error::IncorrectPadding)
}

#[allow(dead_code)]
pub fn parse_u8(src: &mut dyn Read) -> Result<u8, Error> {
    src.read_u8().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn parse_u32(src: &mut dyn Read) -> Result<u32, Error> {
    src.read_u32::<BigEndian>().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn parse_u64(src: &mut dyn Read) -> Result<u64, Error> {
    src.read_u64::<BigEndian>().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn parse_bool(src: &mut dyn Read) -> Result<bool, Error> {
    let discr = parse_u32(src)?;
    match discr {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn parse_option<T>(
    src: &mut dyn Read,
    cont: impl FnOnce(&mut dyn Read) -> Result<T, Error>,
) -> Result<Option<T>, Error> {
    let disc = parse_bool(src)?;
    match disc {
        true => Ok(Some(cont(src)?)),
        false => Ok(None),
    }
}

#[allow(dead_code)]
pub fn parse_array<const N: usize, T>(
    src: &mut dyn Read,
    cont: impl Fn(&mut dyn Read) -> Result<T, Error>,
) -> Result<[T; N], Error> {
    let mut buf: [MaybeUninit<T>; N] = [const { MaybeUninit::uninit() }; N];
    for elem in buf.iter_mut() {
        elem.write(cont(src)?);
    }
    read_padding(src, N * size_of::<T>())?;
    Ok(unsafe { buf.as_ptr().cast::<[T; N]>().read() })
}

#[allow(dead_code)]
pub fn parse_vector<T>(
    src: &mut dyn Read,
    cont: impl Fn(&mut dyn Read) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let size = parse_u32_as_usize(src)?;
    let mut vec = Vec::with_capacity(size);
    for _ in 0..size {
        vec.push(cont(src)?);
    }
    read_padding(src, size_of::<T>() * size)?;
    Ok(vec)
}

#[allow(dead_code)]
pub fn parse_vec_max_size<T>(
    src: &mut dyn Read,
    cont: impl Fn(&mut dyn Read) -> Result<T, Error>,
    max_size: usize,
) -> Result<Vec<T>, Error> {
    let size = parse_u32_as_usize(src)?;
    if size > max_size {
        return Err(Error::MaxELemLimit);
    }
    let mut vec = Vec::with_capacity(size);
    for _ in 0..size {
        vec.push(cont(src)?);
    }
    read_padding(src, size_of::<T>() * size)?;
    Ok(vec)
}

#[allow(dead_code)]
pub fn parse_string_max_len(src: &mut dyn Read, max_size: usize) -> Result<String, Error> {
    let vec = parse_vec_max_size(src, |s| parse_u8(s), max_size)?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

#[allow(dead_code)]
pub fn parse_string(src: &mut dyn Read) -> Result<String, Error> {
    let vec = parse_vector(src, |s| parse_u8(s))?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

#[allow(dead_code)]
pub fn parse_c_enum<T: FromPrimitive>(src: &mut dyn Read) -> Result<T, Error> {
    let val = FromPrimitive::from_u32(parse_u32(src)?);
    match val {
        Some(res) => Ok(res),
        None => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
fn parse_u32_as_usize(src: &mut dyn Read) -> Result<usize, Error> {
    match parse_u32(src)?.to_usize() {
        None => Err(Error::ImpossibleTypeCast),
        Some(n) => Ok(n),
    }
}
