//! Implements parsing for [`read_dir_plus::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u32;
use crate::parser::Result;
use crate::vfs::read_dir;
use crate::vfs::read_dir_plus;

/// Parses a [`read_dir::Cookie`] from the provided `Read` source.
pub fn cookie(_src: &mut impl Read) -> Result<read_dir::Cookie> {
    todo!()
}

/// Parses a [`read_dir::CookieVerifier`] from the provided `Read` source.
pub fn cookie_verifier(_src: &mut impl Read) -> Result<read_dir::CookieVerifier> {
    todo!()
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
    use std::io::Cursor;

    // not ready to run yet - no cookies
    #[allow(dead_code)]
    fn test_readdir_plus() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x08, 0x00,
            0x00, 0x00, 0x10, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        // TODO(cookie and cookie_verifier)
        assert_eq!(result.dir_count, 2048);
        assert_eq!(result.max_count, 4096);
    }
}
