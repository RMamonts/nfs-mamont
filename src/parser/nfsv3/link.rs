//! Implements parsing for [`link::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::{file, MAX_FILENAME};
use crate::parser::primitive::string_max_size;
use crate::parser::Result;
use crate::vfs::link;

pub fn args(src: &mut impl Read) -> Result<link::Args> {
    Ok(link::Args { file: file::handle(src)?, dir: file::handle(src)?, name: string_max_size(src, MAX_FILENAME)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn test_link() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x00, 0x00, 0x00, 0x04, b'l', b'i', b'n', b'k',
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.file.0, [0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10]);
        assert_eq!(result.name, "link");
    }
}
