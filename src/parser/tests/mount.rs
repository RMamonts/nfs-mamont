use std::io::Cursor;

use crate::mount::mnt::MountArgs;
use crate::mount::umnt::UnmountArgs;
use crate::mount::MOUNT_DIRPATH_LEN;
use crate::parser::mount::mnt::mount;
use crate::parser::mount::umnt::unmount;
use crate::vfs::file;

#[test]
fn test_mount_basic() {
    #[rustfmt::skip]
    let mut data = Cursor::new(vec![
        // String length (u32, Big Endian) = 6
        0x00, 0x00, 0x00, 0x06,
        // String contents (6 bytes) = /mnt/1
        b'/', b'm', b'n', b't', b'/', b'1',
        // Padding (alignment up to 4 bytes)
        0x00, 0x00,
    ]);

    let result = mount(&mut data).unwrap();
    let expected = MountArgs(file::Path::new(String::from("/mnt/1")).unwrap());
    assert_eq!(result, expected);
}

#[test]
fn test_unmount_basic() {
    #[rustfmt::skip]
    let mut data = Cursor::new(vec![
        // String length (u32, Big Endian) = 8
        0x00, 0x00, 0x00, 0x08,
        // String contents (8 bytes) = /tmp/tes
        b'/', b't', b'm', b'p', b'/', b't', b'e', b's',
    ]);

    let result = unmount(&mut data).unwrap();
    let expected = UnmountArgs(file::Path::new(String::from("/tmp/tes")).unwrap());
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
    #[rustfmt::skip]
    let mut data = Cursor::new(vec![
        // String length (u32, Big Endian) = 5
        0x00, 0x00, 0x00, 0x05,
        // String contents (6 bytes) = /tmp
        b'/', b't', b'm', b'p',
    ]);

    let result = unmount(&mut data);
    assert!(result.is_err());
}

#[test]
fn test_mount_unaligned_path() {
    #[rustfmt::skip]
    let mut data = Cursor::new(vec![
        // String length (u32, Big Endian) = 3
        0x00, 0x00, 0x00, 0x03,
        // String contents (6 bytes) = /vm
        b'/', b'v', b'm',
    ]);

    let result = mount(&mut data);
    assert!(result.is_err());
}
