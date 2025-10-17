use super::ParserError;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Read;

pub const ALIGNMENT: usize = 4;

#[allow(unused)]
pub trait ToParse: Sized {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError>;
}

#[allow(unused)]
pub fn parse<T: ToParse, R: Read>(src: &mut R) -> Result<T, ParserError> {
    T::parse(src)
}

pub fn read_padding<R: Read>(src: &mut R, n: usize) -> Result<(), ParserError> {
    let mut buf = [0u8; ALIGNMENT];
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    src.read_exact(&mut buf[..padding]).map_err(|_| ParserError::IncorrectPadding)
}

impl ToParse for u32 {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        src.read_u32::<BigEndian>().map_err(|_| ParserError::ReadError)
    }
}

impl ToParse for u64 {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        src.read_u64::<BigEndian>().map_err(|_| ParserError::ReadError)
    }
}

impl ToParse for bool {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let discr = parse::<u32, R>(src)?;
        match discr {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ParserError::EnumDiscMismatch),
        }
    }
}

impl<T: ToParse> ToParse for Option<T> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let disc = parse::<bool, R>(src)?;
        match disc {
            true => Ok(Some(T::parse(src)?)),
            false => Ok(None),
        }
    }
}

impl<const N: usize> ToParse for [u8; N] {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let mut buf = vec![0u8; N];
        src.read_exact(&mut buf).map_err(|_| ParserError::ReadError)?;
        read_padding(src, N)?;
        let res = unsafe { Box::from_raw(Box::into_raw(buf.into_boxed_slice()) as *mut [u8; N]) };
        Ok(*res)
    }
}

impl<const N: usize, T: ToParse> ToParse for [T; N] {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let mut buf = Vec::<T>::with_capacity(N);
        for _ in 0..N {
            buf.push(T::parse(src)?);
        }
        let res = unsafe { Box::from_raw(Box::into_raw(buf.into_boxed_slice()) as *mut [T; N]) };
        Ok(*res)
    }
}

impl ToParse for Vec<u8> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let size = u32::parse(src)?;
        let mut vec = vec![0u8; size as usize];
        src.read_exact(&mut vec).map_err(|_| ParserError::ReadError)?;
        read_padding(src, size as usize)?;
        Ok(vec)
    }
}

impl<T: ToParse> ToParse for Vec<T> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let size = u32::parse(src)?;
        let mut vec = Vec::with_capacity(size as usize);
        for _ in 0..size {
            vec.push(T::parse(src)?);
        }
        Ok(vec)
    }
}

#[allow(dead_code)]
pub struct StringWithMaxLength<const N: usize>(String);

impl<const N: usize> ToParse for StringWithMaxLength<N> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let size = u32::parse(src)?;
        if size as usize > N {
            return Err(ParserError::StringTooLong);
        }
        let mut vec = vec![0u8; size as usize];
        src.read_exact(&mut vec).map_err(|_| ParserError::ReadError)?;
        read_padding(src, size as usize)?;
        Ok(StringWithMaxLength(String::from_utf8(vec).map_err(|_| ParserError::IncorrectString)?))
    }
}

impl ToParse for String {
    fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
        let size = u32::parse(src)?;
        let mut vec = vec![0u8; size as usize];
        src.read_exact(&mut vec).map_err(|_| ParserError::ReadError)?;
        read_padding(src, size as usize)?;
        String::from_utf8(vec).map_err(|_| ParserError::IncorrectString)
    }
}

#[macro_export]
macro_rules! parse_struct {
    ($t:ident, $($element:ident),*) => {
        impl ToParse for $t {
            fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
                Ok($t {$($element: parse(src)?,)*})
            }
        }
    };
}

#[macro_export]
macro_rules! parse_enum {
    ($enum:ident; $($variant:ident = $disc:expr),* $(,)?) => {
        impl ToParse for $enum {
            fn parse<R: Read>(src: &mut R) -> Result<Self, ParserError> {
                let disc = u32::parse(src)?;
                match disc {
                    $(
                        $disc => Ok(Self::$variant),
                    )*
                    _ => Err(ParserError::EnumDiscMismatch),
                }
            }
        }
    };
}
