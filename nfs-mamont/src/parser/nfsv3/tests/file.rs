use std::io::Cursor;

use crate::parser::nfsv3::file::{device, file_name, file_path, handle, r#type, time, wcc_attr};
use crate::parser::Error;
use crate::vfs::file;
use crate::vfs::file::Time;

#[test]
fn test_parse_device_success() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02
    ];

    let result = device(&mut Cursor::new(DATA)).unwrap();

    assert_eq!(result.major, 1);
    assert_eq!(result.minor, 2);
}

#[test]
fn test_device_error() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[0x00, 0x00, 0x01];
    let mut src = Cursor::new(DATA);

    assert!(matches!(device(&mut src), Err(Error::IO(_))));
}

#[test]
fn test_nfstime_success() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02
    ];

    let result = time(&mut Cursor::new(DATA)).unwrap();

    assert_eq!(result.seconds, 1);
    assert_eq!(result.nanos, 2);
}

#[test]
fn test_nfstime_error() {
    const DATA: &[u8] = &[0x00, 0x00, 0x01];

    assert!(matches!(time(&mut Cursor::new(&DATA)), Err(Error::IO(_))));
}

#[test]
fn test_nfs_fh3_success() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x09, 0x00, 0x01, 0x02, 0x03,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let result = handle(&mut Cursor::new(DATA)).unwrap();

    assert_eq!(result.0, [0x00, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]);
}

#[test]
fn test_nfs_fh3_badfh() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00,
        0x00, 0x00, 0x00, 0x00
    ];

    let result = handle(&mut Cursor::new(DATA));

    assert!(matches!(result, Err(Error::BadFileHandle)));
}

#[test]
fn test_type_regular() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x01];

    let result = r#type(&mut Cursor::new(DATA)).unwrap();
    assert!(matches!(result, file::Type::Regular));
}

#[test]
fn test_type_dir() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x02];

    let result = r#type(&mut Cursor::new(DATA)).unwrap();
    assert!(matches!(result, file::Type::Directory));
}

#[test]
fn test_type_block() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x02,
    ];

    let result = r#type(&mut Cursor::new(DATA)).unwrap();
    assert!(matches!(result, file::Type::BlockDevice));
}

#[test]
fn test_type_symlink() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x05];

    let result = r#type(&mut Cursor::new(DATA)).unwrap();
    assert!(matches!(result, file::Type::Symlink));
}

#[test]
fn test_type_failure() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x08];

    assert!(matches!(r#type(&mut Cursor::new(DATA)), Err(Error::EnumDiscMismatch)));
}

#[test]
fn test_file_path_success() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x04, b'f', b'i', b'l', b'e'];
    let file = file::Path::new("file".to_string()).unwrap();
    assert_eq!(file_path(&mut Cursor::new(DATA)).unwrap(), file);
}

#[test]
fn test_file_path_padding_error() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x02, b'f', b'i', 0x00];
    assert!(matches!(file_path(&mut Cursor::new(DATA)), Err(Error::IO(_))));
}

#[test]
fn test_file_name_success() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x04, b'f', b'i', b'l', b'e'];
    let file = file::Name::new("file".to_string()).unwrap();
    assert_eq!(file_name(&mut Cursor::new(DATA)).unwrap(), file);
}

#[test]
fn test_file_name_too_long_error() {
    const DATA: &[u8] = &[0x00, 0x00, 0x01, 0x00];
    assert!(matches!(file_name(&mut Cursor::new(DATA)), Err(Error::MaxElemLimit)));
}

#[test]
fn test_file_name_empty_error() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x00];
    assert!(matches!(file_name(&mut Cursor::new(DATA)), Err(Error::IO(_))));
}

#[test]
fn test_file_name_separator_error() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x03, b'a', b'/', b'b', 0x00];
    assert!(matches!(file_name(&mut Cursor::new(DATA)), Err(Error::IO(_))));
}

#[test]
fn test_file_path_too_long_error() {
    const DATA: &[u8] = &[0x00, 0x00, 0x04, 0x01];
    assert!(matches!(file_path(&mut Cursor::new(DATA)), Err(Error::MaxElemLimit)));
}

#[test]
fn test_file_path_empty_error() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x00];
    assert!(matches!(file_path(&mut Cursor::new(DATA)), Err(Error::IO(_))));
}

#[test]
fn test_wcc_attr_success() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x52,
        0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x01, 0x01,
        0x00, 0x00, 0x00, 0xA0, 0x00, 0x00, 0x05, 0x23,
    ];

    let expected = file::WccAttr {
        size: 82,
        mtime: Time { seconds: 15, nanos: 257 },
        ctime: Time { seconds: 160, nanos: 1315 },
    };

    assert_eq!(wcc_attr(&mut Cursor::new(DATA)).unwrap(), expected);
}
