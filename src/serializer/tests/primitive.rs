use std::io::Cursor;

use crate::serializer::{array, bool, option, string, string_max_size, u32, u64, vector};

#[test]
fn test_u32() {
    let mut init = Cursor::new([0u8; 4]);
    u32(&mut init, 12).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 12])
}

#[test]
fn test_u64() {
    let mut init = Cursor::new([0u8; 8]);
    u64(&mut init, 256).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 0, 0, 0, 1, 0])
}

#[test]
fn test_bool() {
    let mut init = Cursor::new([0u8; 8]);
    bool(&mut init, true).unwrap();
    bool(&mut init, false).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 1, 0, 0, 0, 0])
}

#[test]
fn test_option() {
    let mut init = Cursor::new([0u8; 12]);
    option(&mut init, None, |t, init| u32(init, t)).unwrap();
    option(&mut init, Some(32), |t, init| u32(init, t)).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 32])
}

#[test]
fn test_array() {
    let mut init = Cursor::new([0u8, 0u8, 0u8, 1u8, 5u8, 5u8]);
    array(&mut init, [7u8, 255, 64]).unwrap();
    assert_eq!(init.into_inner(), [7, 255, 64, 0, 5, 5])
}

#[test]
fn test_vector() {
    let mut init = Cursor::new([1u8; 13]);
    vector(&mut init, vec![7u8, 255, 64, 0, 64, 78, 12].as_slice()).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 7, 7, 255, 64, 0, 64, 78, 12, 0, 1])
}

#[test]
fn test_string() {
    let mut init = Cursor::new([1u8; 13]);
    string(&mut init, "test42".to_string()).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 6, b't', b'e', b's', b't', b'4', b'2', 0, 0, 1])
}

#[test]
fn test_string_max() {
    let mut init = Cursor::new([1u8; 13]);
    string_max_size(&mut init, "test42".to_string(), 7).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 6, b't', b'e', b's', b't', b'4', b'2', 0, 0, 1])
}

#[test]
fn test_string_max_error() {
    let mut init = Cursor::new([1u8; 13]);
    let res = string_max_size(&mut init, "test42".to_string(), 5);
    assert!(res.is_err())
}

#[test]
fn test_write_error() {
    let mut init = Cursor::new([0u8; 1]);
    let res = u32(&mut init, 1);
    assert!(res.is_err())
}
