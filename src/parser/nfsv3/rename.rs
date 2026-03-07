//! Implements parsing for [`rename::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::rename;

pub fn args(src: &mut impl Read) -> Result<rename::Args> {
    Ok(rename::Args {
        from: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        to: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn test_rename() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x04,
            b'o', b'l', b'd', b'n', 0x00, 0x00, 0x00, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x00, 0x00, 0x00, 0x04, b'n', b'e', b'w', b'n',
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.from.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.from.name.as_str(), "oldn");
        assert_eq!(result.to.dir.0, [0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10]);
        assert_eq!(result.to.name.as_str(), "newn");
    }
}
