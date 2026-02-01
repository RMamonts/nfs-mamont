//! Implements parsing for [`rm_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::{file, MAX_FILENAME};
use crate::parser::primitive::string_max_size;
use crate::parser::Result;
use crate::vfs::rm_dir;

pub fn args(src: &mut impl Read) -> Result<rm_dir::Args> {
    Ok(rm_dir::Args { dir: file::handle(src)?, name: string_max_size(src, MAX_FILENAME)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn test_rmdir() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x04,
            b'd', b'i', b'r', b'1',
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.name, "dir1");
    }
}
