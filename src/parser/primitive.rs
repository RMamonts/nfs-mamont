use std::io::Read;
use std::mem::MaybeUninit;

use byteorder::{BigEndian, ReadBytesExt};
use num_traits::{FromPrimitive, ToPrimitive};

use super::Error;

#[allow(dead_code)]
pub const ALIGNMENT: usize = 4;

#[allow(dead_code)]
fn read_padding(src: &mut (impl Read + ?Sized), n: usize) -> Result<(), Error> {
    let mut buf = [0u8; ALIGNMENT];
    let padding = (ALIGNMENT - n % ALIGNMENT) % ALIGNMENT;
    src.read_exact(&mut buf[..padding]).map_err(|_| Error::IncorrectPadding)
}

#[allow(dead_code)]
pub fn parse_u8(src: &mut (impl Read + ?Sized)) -> Result<u8, Error> {
    src.read_u8().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn parse_u32(src: &mut (impl Read + ?Sized)) -> Result<u32, Error> {
    src.read_u32::<BigEndian>().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn parse_u64(src: &mut (impl Read + ?Sized)) -> Result<u64, Error> {
    src.read_u64::<BigEndian>().map_err(Error::IO)
}

#[allow(dead_code)]
pub fn parse_bool(src: &mut (impl Read + ?Sized)) -> Result<bool, Error> {
    let discr = parse_u32(src)?;
    match discr {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
pub fn parse_option<T>(
    src: &mut impl Read,
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
    src: &mut impl Read,
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
    src: &mut impl Read,
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
pub fn parse_vec_max_size<const N: usize, T>(
    src: &mut impl Read,
    cont: impl Fn(&mut dyn Read) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let size = parse_u32_as_usize(src)?;
    if size > N {
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
pub fn parse_string_max_len<const N: usize>(src: &mut impl Read) -> Result<String, Error> {
    let vec = parse_vec_max_size::<N, u8>(src, |s| parse_u8(s))?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

#[allow(dead_code)]
pub fn parse_string(src: &mut impl Read) -> Result<String, Error> {
    let vec = parse_vector(src, |s| parse_u8(s))?;
    String::from_utf8(vec).map_err(Error::IncorrectString)
}

#[allow(dead_code)]
pub fn parse_c_enum<T: FromPrimitive>(src: &mut impl Read) -> Result<T, Error> {
    let val = FromPrimitive::from_u32(parse_u32(src)?);
    match val {
        Some(res) => Ok(res),
        None => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
fn parse_u32_as_usize(src: &mut impl Read) -> Result<usize, Error> {
    match parse_u32(src)?.to_usize() {
        None => Err(Error::TypeConv),
        Some(n) => Ok(n),
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use byteorder::{BigEndian, WriteBytesExt};

    use crate::parser::primitive::{
        parse_array, parse_bool, parse_option, parse_string, parse_string_max_len, parse_u32,
        parse_u64, parse_u8, parse_vector,
    };
    use crate::parser::Error;

    #[test]
    fn test_u32() {
        let init = [0u32, 7, 788965];
        let mut src = Vec::with_capacity(size_of::<u32>() * init.len());
        for i in init {
            src.write_u32::<BigEndian>(i).unwrap();
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = parse_u32(&mut src).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_u64() {
        let init = [2u64, 0, 125, 78569];
        let mut src = Vec::with_capacity(size_of::<u64>() * init.len());
        for i in init {
            src.write_u64::<BigEndian>(i).unwrap();
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = parse_u64(&mut src).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_bool() {
        let init = [true, false, true];
        let mut src = Vec::with_capacity(size_of::<u32>() * init.len());
        for i in init {
            src.write_u32::<BigEndian>(if i { 1 } else { 0 }).unwrap();
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = parse_bool(&mut src).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_option() {
        let init = [None, Some(85u32), Some(0)];
        let mut src = Vec::new();
        for op in init {
            if let Some(val) = op {
                src.write_u32::<BigEndian>(1).unwrap();
                src.write_u32::<BigEndian>(val).unwrap();
            } else {
                src.write_u32::<BigEndian>(0).unwrap();
            }
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = parse_option(&mut src, |s| parse_u32(s)).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_array_u32() {
        let init = [457u32, 475, 0];
        let mut src = Vec::new();
        init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        let mut src = Cursor::new(src);
        let val = parse_array::<3, u32>(&mut src, |s| parse_u32(s)).expect("Cannot parse value!");
        assert_eq!(val, init)
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

        let result = parse_vector(&mut Cursor::new(src), |s| parse_u8(s)).unwrap();
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
        let result = parse_vector(&mut Cursor::new(src), |s| parse_u8(s)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_u8_array_padding_error() {
        let init = [1u8, 2, 3];
        let mut src = Vec::new();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        let result = parse_array::<3, u8>(&mut Cursor::new(src), |s| parse_u8(s));
        assert!(matches!(result, Err(Error::IncorrectPadding)));
    }

    #[test]
    fn test_u8_array_miss_elements() {
        let init = [78u32, 0, 78965];
        let mut src = Vec::new();
        let _ = init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        let result = parse_array::<4, u32>(&mut Cursor::new(src), |s| parse_u32(s));
        assert!(matches!(result, Err(Error::IO(_))));
    }

    #[test]
    fn test_vec_u32() {
        let init = vec![457u32, 475, 0, 42];
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(init.len() as u32).unwrap();
        for i in &init {
            src.write_u32::<BigEndian>(*i).unwrap();
        }
        let result = parse_vector(&mut Cursor::new(src), |s| parse_u32(s)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_string_utf8_error() {
        let mut src = Vec::new();
        let invalid_utf8 = vec![0xFF, 0xFF, 0xFF];
        src.write_u32::<BigEndian>(invalid_utf8.len() as u32).unwrap();
        src.extend_from_slice(&invalid_utf8);
        src.push(0);
        let result = parse_string(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::IncorrectString(_))));
    }

    #[test]
    fn test_string_valid() {
        let test_string = "test string".to_string();
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        src.write_u8(0u8).unwrap();
        let result = parse_string(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, test_string);
    }

    #[test]
    fn test_string_with_max_len_valid() {
        let test_string = "test".to_string();
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        let result = parse_string_max_len::<10>(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, test_string);
    }

    #[test]
    fn test_string_with_max_len_too_long() {
        let test_string = "this string is too long".to_string();
        let mut src = Vec::new();

        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        let padding_len = (4 - (test_string.len() % 4)) % 4;
        src.extend(vec![0u8; padding_len]);

        let result = parse_string_max_len::<10>(&mut Cursor::new(src));
        println!("{:?}", result);
        assert!(matches!(result, Err(Error::MaxELemLimit)));
    }

    #[test]
    fn test_read_error() {
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(10).unwrap();
        let result = parse_vector(&mut Cursor::new(src), |s| parse_u8(s));
        assert!(matches!(result, Err(Error::IO(_))));
    }
}
