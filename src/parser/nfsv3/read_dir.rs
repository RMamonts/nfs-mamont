//! Implements parsing for [`read_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::read_dir_plus::cookie;
use crate::parser::nfsv3::read_dir_plus::cookie_verifier;
use crate::parser::primitive::u32;
use crate::parser::Result;
use crate::vfs::read_dir;

/// Parses the arguments for an NFSv3 `READDIR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<read_dir::Args> {
    Ok(read_dir::Args {
        dir: file::handle(src)?,
        cookie: cookie(src)?,
        cookie_verifier: cookie_verifier(src)?,
        count: u32(src)?,
    })
}

#[cfg(test)]
mod tests {
    use crate::vfs::read_dir;
    use std::io::Cursor;

    #[test]
    fn test_readdir() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            // dir file handle length = 8
            0x00, 0x00, 0x00, 0x08,
            // dir file handle bytes
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            // cookie = 4096 (u64, BE)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
            // cookie_verifier = 8192 bytes marker
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00,
            // count = 2048 (u32, BE)
            0x00, 0x00, 0x08, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.cookie, read_dir::Cookie::new(4096));
        assert_eq!(
            result.cookie_verifier,
            read_dir::CookieVerifier::new([0, 0, 0, 0, 0, 0, 0x20, 0])
        );
        assert_eq!(result.count, 2048);
    }

    #[test]
    fn test_readdir_unaligned_after_fh() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            // dir file handle length = 7
            0x00, 0x00, 0x00, 0x07,
            // dir file handle bytes
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            // bad/unexpected padding tail to provoke parser error
            0x00,
            // cookie = 4096 (u64, BE)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
            // cookie_verifier = 8192 bytes marker
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00,
            // count = 2048 (u32, BE) + one extra byte to keep malformed layout
            0x00, 0x00, 0x08, 0x00, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA));
        assert!(result.is_err());
    }
}
