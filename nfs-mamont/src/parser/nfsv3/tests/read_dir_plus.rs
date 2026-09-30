use std::io::Cursor;

use crate::parser::nfsv3::read_dir_plus::args;
use crate::vfs::read_dir;

#[test]
fn test_readdir_plus() {
    #[rustfmt::skip]
    const DATA: &[u8] = &[
        // dir file handle length = 9
        0x00, 0x00, 0x00, 0x09,
        // dir file handle bytes: backend index + object payload
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        // xdr padding of the file handle
        0x00, 0x00, 0x00,
        // cookie = 4096 (u64, BE)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
        // cookie_verifier = 8192 bytes marker
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00,
        // dir_count = 2048 (u32, BE)
        0x00, 0x00, 0x08, 0x00,
        // max_count = 4096 (u32, BE)
        0x00, 0x00, 0x10, 0x00,
    ];

    let result = args(&mut Cursor::new(DATA)).unwrap();

    assert_eq!(result.dir.0, [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    assert_eq!(result.cookie, read_dir::Cookie::new(4096));
    assert_eq!(result.cookie_verifier, read_dir::CookieVerifier::new([0, 0, 0, 0, 0, 0, 0x20, 0]));
    assert_eq!(result.dir_count, 2048);
    assert_eq!(result.max_count, 4096);
}
