//! Implements parsing for [`read_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::read_dir_plus::cookie;
use crate::parser::nfsv3::read_dir_plus::cookie_verifier;
use crate::parser::primitive::u32;
use crate::parser::Result;
use crate::vfs::read_dir;

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
    use std::io::Cursor;

    #[test]
    fn test_readdir() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x08, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        // assert_eq!(result.cookie, ) TODO()
        // assert_eq!(result.cookie_verifier, ) TODO()
        assert_eq!(result.count, 2048);
    }

    #[test]
    fn test_readdir_unaligned_after_fh() {
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x07, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00,
            0x00, 0x08, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA));
        assert!(result.is_err());
    }
}
