use std::io::Cursor;

use byteorder::{BigEndian, WriteBytesExt};

use crate::parser::primitive::{array, string, string_max_size, vector};
use crate::parser::Error;

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

    let result = vector(&mut Cursor::new(src)).unwrap();
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
    let result = vector(&mut Cursor::new(src)).unwrap();
    assert_eq!(result, init);
}

#[test]
fn test_u8_array_padding_error() {
    let init = [1u8, 2, 3];
    let mut src = Vec::new();
    for i in &init {
        src.write_u8(*i).unwrap();
    }
    let result = array::<3>(&mut Cursor::new(src));
    assert!(matches!(result, Err(Error::IncorrectPadding)));
}

#[test]
fn test_u8_array_miss_elements() {
    let init = [78u8, 0, 255];
    let mut src = Vec::new();
    let _ = init.map(|i| src.write_u8(i).unwrap());
    let result = array::<4>(&mut Cursor::new(src));
    assert!(matches!(result, Err(Error::IO(_))));
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
    let result = vector(&mut Cursor::new(src));
    assert!(matches!(result, Err(Error::IO(_))));
}
