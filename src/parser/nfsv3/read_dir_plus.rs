//! Implements parsing for [`read_dir_plus::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::{array, u32, u64};
use crate::parser::Result;
use crate::vfs::{read_dir, read_dir_plus};

/// Parses a [`read_dir::Cookie`] from the provided `Read` source.
pub fn cookie(src: &mut impl Read) -> Result<read_dir::Cookie> {
    Ok(read_dir::Cookie::new(u64(src)?))
}

/// Parses a [`read_dir::CookieVerifier`] from the provided `Read` source.
pub fn cookie_verifier(src: &mut impl Read) -> Result<read_dir::CookieVerifier> {
    Ok(read_dir::CookieVerifier::new(array(src)?))
}

/// Parses the arguments for an NFSv3 `READDIRPLUS` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<read_dir_plus::Args> {
    Ok(read_dir_plus::Args {
        dir: file::handle(src)?,
        cookie: cookie(src)?,
        cookie_verifier: cookie_verifier(src)?,
        dir_count: u32(src)?,
        max_count: u32(src)?,
    })
}

#[cfg(test)]
mod tests {
    use crate::vfs::read_dir;
    use std::io::Cursor;

    #[test]
    fn test_readdir_plus() {
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
            // dir_count = 2048 (u32, BE)
            0x00, 0x00, 0x08, 0x00,
            // max_count = 4096 (u32, BE)
            0x00, 0x00, 0x10, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.cookie, read_dir::Cookie::new(4096));
        assert_eq!(
            result.cookie_verifier,
            read_dir::CookieVerifier::new([0, 0, 0, 0, 0, 0, 0x20, 0])
        );
        assert_eq!(result.dir_count, 2048);
        assert_eq!(result.max_count, 4096);
    }
}
