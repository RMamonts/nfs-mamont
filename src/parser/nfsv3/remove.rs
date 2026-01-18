//! Implements parsing for [`remove::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::remove;

pub fn args(src: &mut impl Read) -> Result<remove::Args> {
    Ok(remove::Args { dir: file::handle(src)?, name: string(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn test_remove() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x04,
            b'f', b'i', b'l', b'e',
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.name, "file");
    }
}
