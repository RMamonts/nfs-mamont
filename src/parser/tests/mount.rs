use std::io::Cursor;

use crate::mount::MOUNT_DIRPATH_LEN;
use crate::mount::{mnt, umnt};
use crate::parser::mount::mnt::mount;
use crate::parser::mount::umnt::unmount;
use crate::vfs::file;

#[test]
fn test_mount_basic() {
    let mut data =
        Cursor::new(vec![0x00, 0x00, 0x00, 0x06, b'/', b'm', b'n', b't', b'/', b'1', 0x00, 0x00]);

    let result = mount(&mut data).unwrap();
    let expected = mnt::Args { dirpath: file::Path::new(String::from("/mnt/1")).unwrap() };
    assert_eq!(result, expected);
}

#[test]
fn test_unmount_basic() {
    let mut data =
        Cursor::new(vec![0x00, 0x00, 0x00, 0x08, b'/', b't', b'm', b'p', b'/', b't', b'e', b's']);

    let result = unmount(&mut data).unwrap();
    let expected = umnt::Args { dirpath: file::Path::new(String::from("/tmp/tes")).unwrap() };
    assert_eq!(result, expected);
}

#[test]
fn test_mount_exceeds_max_length() {
    let oversized_path = vec![b'a'; MOUNT_DIRPATH_LEN + 1];
    let mut data_vec = vec![
        ((MOUNT_DIRPATH_LEN + 1) as u32).to_be_bytes()[0],
        ((MOUNT_DIRPATH_LEN + 1) as u32).to_be_bytes()[1],
        ((MOUNT_DIRPATH_LEN + 1) as u32).to_be_bytes()[2],
        ((MOUNT_DIRPATH_LEN + 1) as u32).to_be_bytes()[3],
    ];
    data_vec.extend_from_slice(&oversized_path);

    let mut data = Cursor::new(data_vec);
    let result = mount(&mut data);
    assert!(result.is_err());
}

#[test]
fn test_unmount_insufficient_data() {
    let mut data = Cursor::new(vec![0x00, 0x00, 0x00, 0x05, b'/', b't', b'm', b'p']);

    let result = unmount(&mut data);
    assert!(result.is_err());
}

#[test]
fn test_mount_unaligned_path() {
    let mut data = Cursor::new(vec![0x00, 0x00, 0x00, 0x03, b'/', b'v', b'm']);

    let result = mount(&mut data);
    assert!(result.is_err());
}
