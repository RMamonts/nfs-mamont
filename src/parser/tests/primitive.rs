#![cfg(test)]

use std::io::Cursor;

use byteorder::{BigEndian, WriteBytesExt};

use crate::parser::primitive::{
    array, bool, option, string, string_max_size, u32, u64, u8, vector,
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
        let val = u32(&mut src).expect("Cannot parse value!");
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
        let val = u64(&mut src).expect("Cannot parse value!");
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
        let val = bool(&mut src).expect("Cannot parse value!");
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
        let val = option(&mut src, |s| u32(s)).expect("Cannot parse value!");
        assert_eq!(val, correct_res)
    }
}

#[test]
fn test_array_u32() {
    let init = [457u32, 475, 0];
    let mut src = Vec::new();
    let _ = init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
    let mut src = Cursor::new(src);
    let val = array::<3, u32>(&mut src, |s| u32(s)).expect("Cannot parse value!");
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

    let result = vector(&mut Cursor::new(src), |s| u8(s)).unwrap();
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
    let result = vector(&mut Cursor::new(src), |s| u8(s)).unwrap();
    assert_eq!(result, init);
}

#[test]
fn test_u8_array_padding_error() {
    let init = [1u8, 2, 3];
    let mut src = Vec::new();
    for i in &init {
        src.write_u8(*i).unwrap();
    }
    let result = array::<3, u8>(&mut Cursor::new(src), |s| u8(s));
    assert!(matches!(result, Err(Error::IncorrectPadding)));
}

#[test]
fn test_u8_array_miss_elements() {
    let init = [78u32, 0, 78965];
    let mut src = Vec::new();
    let _ = init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
    let result = array::<4, u32>(&mut Cursor::new(src), |s| u32(s));
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
    let result = vector(&mut Cursor::new(src), |s| u32(s)).unwrap();
    assert_eq!(result, init);
}

#[test]
fn test_string_utf8_error() {
    let mut src = Vec::new();
    let invalid_utf8 = vec![0xFF, 0xFF, 0xFF];
    src.write_u32::<BigEndian>(invalid_utf8.len() as u32).unwrap();
    src.extend_from_slice(&invalid_utf8);
    src.push(0);
    let result = string(&mut Cursor::new(src));
    assert!(matches!(result, Err(Error::IncorrectString(_))));
}

#[test]
fn test_string_valid() {
    let test_string = "test string".to_string();
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
    src.extend_from_slice(test_string.as_bytes());
    src.write_u8(0u8).unwrap();
    let result = string(&mut Cursor::new(src)).unwrap();
    assert_eq!(result, test_string);
}

#[test]
fn test_string_with_max_len_valid() {
    let test_string = "test".to_string();
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
    src.extend_from_slice(test_string.as_bytes());
    let result = string_max_size(&mut Cursor::new(src), 10).unwrap();
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

    let result = string_max_size(&mut Cursor::new(src), 10);
    assert!(matches!(result, Err(Error::MaxELemLimit)));
}

#[test]
fn test_read_error() {
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(10).unwrap();
    let result = vector(&mut Cursor::new(src), |s| u8(s));
    assert!(matches!(result, Err(Error::IO(_))));
}
