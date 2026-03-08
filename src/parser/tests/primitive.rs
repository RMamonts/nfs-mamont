use std::io::Cursor;

use byteorder::{BigEndian, WriteBytesExt};

use crate::parser::primitive::{
    array, discard_opaque_max_size, string, string_into, string_max_size, vector, vector_into,
};
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
fn test_vector_into_reuses_buffer() {
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(3).unwrap();
    src.extend_from_slice(&[9, 8, 7, 0]);

    let mut buffer = vec![1, 2, 3, 4, 5, 6];
    let old_capacity = buffer.capacity();
    vector_into(&mut Cursor::new(src), &mut buffer).unwrap();

    assert_eq!(buffer, vec![9, 8, 7]);
    assert!(buffer.capacity() >= old_capacity.min(3));
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
fn test_string_into_reuses_buffer() {
    let test_string = "hello".to_string();
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
    src.extend_from_slice(test_string.as_bytes());
    src.extend_from_slice(&[0, 0, 0]);

    let mut buffer = vec![42; 16];
    let parsed = string_into(&mut Cursor::new(src), &mut buffer).unwrap();

    assert_eq!(parsed, test_string);
    assert!(buffer.is_empty());
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
    assert!(matches!(result, Err(Error::MaxElemLimit)));
}

#[test]
fn test_read_error() {
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(10).unwrap();
    let result = vector(&mut Cursor::new(src));
    assert!(matches!(result, Err(Error::IO(_))));
}

#[test]
fn test_discard_opaque_max_size() {
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(5).unwrap();
    src.extend_from_slice(&[1, 2, 3, 4, 5, 0, 0, 0]);

    let mut cursor = Cursor::new(src);
    discard_opaque_max_size(&mut cursor, 8).unwrap();
    assert_eq!(cursor.position() as usize, cursor.get_ref().len());
}

#[test]
fn test_discard_opaque_max_size_too_long() {
    let mut src = Vec::new();
    src.write_u32::<BigEndian>(9).unwrap();
    let result = discard_opaque_max_size(&mut Cursor::new(src), 8);
    assert!(matches!(result, Err(Error::MaxElemLimit)));
}
