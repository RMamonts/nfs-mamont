//! Implements parsing for [`fs_info::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::fs_info;

pub fn args(src: &mut impl Read) -> Result<fs_info::Args> {
    Ok(fs_info::Args { root: file::handle(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn test_fsinfo() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.root.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }
}
