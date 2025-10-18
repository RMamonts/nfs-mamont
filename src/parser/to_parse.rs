use std::io::Read;
use std::mem::MaybeUninit;

use byteorder::{BigEndian, ReadBytesExt};

use super::Error;

pub const ALIGNMENT: usize = 4;

#[allow(unused)]
pub trait ToParse: Sized {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error>;
}

#[allow(unused)]
pub fn parse<T: ToParse, R: Read>(src: &mut R) -> Result<T, Error> {
    T::parse(src)
}

pub fn read_padding<R: Read>(src: &mut R, n: usize) -> Result<(), Error> {
    let mut buf = [0u8; ALIGNMENT];
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    src.read_exact(&mut buf[..padding]).map_err(|_| Error::IncorrectPadding)
}

impl ToParse for u32 {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        src.read_u32::<BigEndian>().map_err(|_| Error::IO)
    }
}

impl ToParse for u64 {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        src.read_u64::<BigEndian>().map_err(|_| Error::IO)
    }
}

impl ToParse for bool {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let discr = parse::<u32, R>(src)?;
        match discr {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::EnumDiscMismatch),
        }
    }
}

impl<T: ToParse> ToParse for Option<T> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let disc = parse::<bool, R>(src)?;
        match disc {
            true => Ok(Some(T::parse(src)?)),
            false => Ok(None),
        }
    }
}

impl<const N: usize> ToParse for [u8; N] {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let mut result = [0; N];
        src.read_exact(&mut result).map_err(|_| Error::IO)?;
        read_padding(src, N)?;
        Ok(result)
    }
}

impl<const N: usize, T: ToParse> ToParse for [T; N] {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let mut buf: [MaybeUninit<T>; N] = [const { MaybeUninit::uninit() }; N];

        for elem in buf.iter_mut() {
            elem.write(T::parse(src)?);
        }
        Ok(unsafe { buf.as_ptr().cast::<[T; N]>().read() })
    }
}

impl ToParse for Vec<u8> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let size = u32::parse(src)?;
        let mut vec = vec![0u8; size as usize];
        src.read_exact(&mut vec).map_err(|_| Error::IO)?;
        read_padding(src, size as usize)?;
        Ok(vec)
    }
}

impl<T: ToParse> ToParse for Vec<T> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let size = u32::parse(src)?;
        let mut vec = Vec::with_capacity(size as usize);
        for _ in 0..size {
            vec.push(T::parse(src)?);
        }
        Ok(vec)
    }
}
#[allow(dead_code)]
pub struct VecWithMaxLen<const N: u32>(Vec<u8>);
impl<const N: u32> ToParse for VecWithMaxLen<N> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let size = u32::parse(src)?;
        if size > N {
            return Err(Error::VecTooLong);
        }
        let mut vec = vec![0u8; size as usize];
        src.read_exact(&mut vec).map_err(|_| Error::IO)?;
        read_padding(src, size as usize)?;
        Ok(VecWithMaxLen(vec))
    }
}

#[allow(dead_code)]
pub struct StringWithMaxLen<const N: u32>(String);

impl<const N: u32> ToParse for StringWithMaxLen<N> {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let size = u32::parse(src)?;
        if size > N {
            return Err(Error::StringTooLong);
        }
        let mut vec = vec![0u8; size as usize];
        src.read_exact(&mut vec).map_err(|_| Error::IO)?;
        read_padding(src, size as usize)?;
        Ok(StringWithMaxLen(String::from_utf8(vec).map_err(|_| Error::IncorrectString)?))
    }
}

impl ToParse for String {
    fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
        let size = u32::parse(src)?;
        let mut vec = vec![0u8; size as usize];
        src.read_exact(&mut vec).map_err(|_| Error::IO)?;
        read_padding(src, size as usize)?;
        String::from_utf8(vec).map_err(|_| Error::IncorrectString)
    }
}

#[macro_export]
macro_rules! parse_struct {
    ($t:ident, $($element:ident),*) => {
        impl ToParse for $t {
            fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
                Ok($t {$($element: parse(src)?,)*})
            }
        }
    };
}

#[macro_export]
macro_rules! parse_enum {
    ($enum:ident; $($variant:ident = $disc:expr),* $(,)?) => {
        impl ToParse for $enum {
            fn parse<R: Read>(src: &mut R) -> Result<Self, Error> {
                let disc = u32::parse(src)?;
                match disc {
                    $(
                        $disc => Ok(Self::$variant),
                    )*
                    _ => Err(Error::EnumDiscMismatch),
                }
            }
        }
    };
}

#[cfg(test)]
mod test {
    use crate::parser::to_parse::{StringWithMaxLen, ToParse};
    use crate::parser::Error;
    use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
    use std::fmt::Debug;
    use std::io::Cursor;

    fn run_test<T: ToParse + PartialEq + Debug>(mut src: Cursor<Vec<u8>>, res: &mut [T]) {
        for correct_res in res {
            let val = T::parse(&mut src).expect("Cannot parse value!");
            assert_eq!(val, *correct_res)
        }
    }
    fn run_test_u8(mut src: Cursor<Vec<u8>>, res: &mut [u8]) {
        for correct_res in res {
            let val = src.read_u8().expect("Cannot read byte!");
            assert_eq!(val, *correct_res)
        }
        assert_eq!(src.position(), src.get_mut().len() as u64)
    }

    #[test]
    fn test_u32() {
        let mut init = [0u32, 7, 788965];
        let mut src = Vec::with_capacity(size_of::<u32>() * init.len());
        for i in init {
            src.write_u32::<BigEndian>(i).unwrap();
        }
        run_test(Cursor::new(src), &mut init)
    }

    #[test]
    fn test_u64() {
        let mut init = [2u64, 0, 125, 78569];
        let mut src = Vec::with_capacity(size_of::<u64>() * init.len());
        for i in init {
            src.write_u64::<BigEndian>(i).unwrap();
        }
        run_test(Cursor::new(src), &mut init)
    }

    #[test]
    fn test_bool() {
        let mut init = [true, false, true];
        let mut src = Vec::with_capacity(size_of::<u32>() * init.len());
        for i in init {
            src.write_u32::<BigEndian>(if i { 1 } else { 0 }).unwrap();
        }
        run_test(Cursor::new(src), &mut init)
    }

    #[test]
    fn test_option() {
        let mut init = [None, Some(85u32), Some(0)];
        let mut src = Vec::new();
        for op in init {
            if let Some(val) = op {
                src.write_u32::<BigEndian>(1).unwrap();
                src.write_u32::<BigEndian>(val).unwrap();
            } else {
                src.write_u32::<BigEndian>(0).unwrap();
            }
        }
        run_test(Cursor::new(src), &mut init);
    }

    #[test]
    fn test_fixed_array() {
        let mut init = [457u32, 475, 0];
        let mut src = Vec::new();
        init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        run_test(Cursor::new(src), &mut init);
    }

    #[test]
    fn test_u8_array() {
        let mut init = [255u8, 1, 4, 7, 0];
        let mut src = Vec::new();
        init.map(|i| src.write_u8(i).unwrap());
        run_test_u8(Cursor::new(src), &mut init);
    }

    #[test]
    fn test_vec_u8() {
        let init = vec![1u8, 2, 3, 4, 5];
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(init.len() as u32).unwrap();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        let padding_len = (4 - (init.len() % 4)) % 4;
        src.extend(vec![0u8; padding_len]);

        let result = Vec::<u8>::parse(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_vec_u8_with_padding() {
        let init = vec![1u8, 2, 3];
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(init.len() as u32).unwrap();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        src.push(0);
        let result = Vec::<u8>::parse(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_u8_array_padding_error() {
        let init = [1u8, 2, 3];
        let mut src = Vec::new();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        let result = <[u8; 3]>::parse(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::IncorrectPadding)));
    }

    #[test]
    fn test_u8_array_correct() {
        let init = [1u8, 2, 3];
        let mut src = Vec::new();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        src.push(4);
        let result = <[u8; 3]>::parse(&mut Cursor::new(src)).unwrap();
        assert_eq!(init, result);
    }

    #[test]
    fn test_u32_array_success() {
        let init = [1u32, 0, 457148];
        let mut src = Vec::new();
        for i in &init {
            src.write_u32::<BigEndian>(*i).unwrap();
        }
        let result = <[u32; 3]>::parse(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_u8_array_miss_elements() {
        let init = [78u32, 0, 78965];
        let mut src = Vec::new();
        let _ = init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        let result = <[u32; 4]>::parse(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::IO)));
    }

    #[test]
    fn test_vec_u32() {
        let init = vec![457u32, 475, 0, 42];
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(init.len() as u32).unwrap();
        for i in &init {
            src.write_u32::<BigEndian>(*i).unwrap();
        }
        let result = Vec::<u32>::parse(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_vec_of_string() {
        let data = vec!["hello".to_string(), "world".to_string(), "test".to_string()];
        let mut src = Vec::new();

        src.write_u32::<BigEndian>(data.len() as u32).unwrap();
        for s in &data {
            src.write_u32::<BigEndian>(s.len() as u32).unwrap();
            src.extend_from_slice(s.as_bytes());
            let padding_len = (4 - (s.len() % 4)) % 4;
            src.extend(vec![0u8; padding_len]);
        }
        let result = Vec::<String>::parse(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, data);
    }
    #[test]
    fn test_string_utf8_error() {
        let mut src = Vec::new();
        let invalid_utf8 = vec![0xFF, 0xFF, 0xFF];
        src.write_u32::<BigEndian>(invalid_utf8.len() as u32).unwrap();
        src.extend_from_slice(&invalid_utf8);
        src.push(0);
        let result = String::parse(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::IncorrectString)));
    }

    #[test]
    fn test_string_with_max_len_valid() {
        let test_string = "test".to_string();
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        let result = StringWithMaxLen::<10>::parse(&mut Cursor::new(src)).unwrap();
        assert_eq!(result.0, test_string);
    }

    #[test]
    fn test_string_with_max_len_too_long() {
        let test_string = "this string is too long".to_string();
        let mut src = Vec::new();

        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        let padding_len = (4 - (test_string.len() % 4)) % 4;
        src.extend(vec![0u8; padding_len]);

        let result = StringWithMaxLen::<10>::parse(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::StringTooLong)));
    }

    #[test]
    fn test_read_error() {
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(10).unwrap();
        let result = Vec::<u8>::parse(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::IO)));
    }
}
