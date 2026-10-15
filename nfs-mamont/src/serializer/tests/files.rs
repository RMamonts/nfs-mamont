use std::io::Cursor;

use crate::serializer::files::{file_handle, file_name, file_path, file_type, nfs_time, wcc_attr};
use crate::vfs::file;

#[test]
fn test_nfstime_success() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x02,
        0x01
    ];

    let mut buffer = Cursor::new([1u8; 9]);

    let time = file::Time { seconds: 1, nanos: 2 };

    nfs_time(&mut buffer, time).unwrap();

    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_nfs_fh3_success() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x09,
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x00, 0x00, 0x00,
        0x01
    ];

    let mut buffer = Cursor::new([1u8; 17]);

    let handle = file::Handle([0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);

    file_handle(&mut buffer, handle).unwrap();

    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_type_regular() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x01, 0x01];

    let mut buffer = Cursor::new([1u8; 5]);

    file_type(&mut buffer, file::Type::Regular).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_type_dir() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x02, 0x01];

    let mut buffer = Cursor::new([1u8; 5]);

    file_type(&mut buffer, file::Type::Directory).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_type_symlink() {
    const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x05, 0x01];

    let mut buffer = Cursor::new([1u8; 5]);

    file_type(&mut buffer, file::Type::Symlink).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_file_path_with_padding() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x05,
        b'd', b'i', b'r', b'/',
        b'0', 0x00, 0x00, 0x00,
        0x01
    ];

    let mut buffer = Cursor::new([1u8; 13]);
    let file = file::Path::new("dir/0".to_string()).unwrap();
    file_path(&mut buffer, file).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_file_path_without_padding() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x04,
        b'/', b'd', b'/', b'e',
        0x01
    ];

    let mut buffer = Cursor::new([1u8; 9]);
    let file = file::Path::new("/d/e".to_string()).unwrap();
    file_path(&mut buffer, file).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_file_name_without_padding() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x04,
        b'f', b'i', b'l', b'e',
        0x01
    ];

    let file = file::Name::new("file".to_string()).unwrap();

    let mut buffer = Cursor::new([1u8; 9]);
    file_name(&mut buffer, file).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_file_name_with_padding() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x05,
        b'f', b'i', b'l', b'e',
        b'0', 0x00, 0x00, 0x00,
        0x01
    ];

    let mut buffer = Cursor::new([1u8; 13]);
    let file = file::Name::new("file0".to_string()).unwrap();
    file_name(&mut buffer, file).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}

#[test]
fn test_wcc_attr_success() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x52,
        0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x01, 0x01,
        0x00, 0x00, 0x00, 0xA0, 0x00, 0x00, 0x05, 0x23,
    ];

    let attr = file::WccAttr {
        size: 82,
        mtime: file::Time { seconds: 15, nanos: 257 },
        ctime: file::Time { seconds: 160, nanos: 1315 },
    };

    let mut buffer = Cursor::new([1u8; 24]);
    wcc_attr(&mut buffer, attr).unwrap();
    assert_eq!(buffer.into_inner(), DATA);
}
