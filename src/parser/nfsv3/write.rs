//! Implements parsing for [`write::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u64;
use crate::parser::primitive::u8;
use crate::parser::primitive::vector;
use crate::parser::Result;
use crate::vfs::write;

fn stable_how(_src: &mut impl Read) -> Result<write::StableHow> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<write::Args> {
    Ok(write::Args {
        file: file::handle(src)?,
        offset: u64(src)?,
        stable: stable_how(src)?,
        data: vector(src, |s| u8(s))?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::vfs::write;

    #[test]
    fn test_write() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
            0x11, 0x22, 0x33, 0x44,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.file.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.offset, 65536);
        assert_eq!(result.data.len(), 1024);
        assert!(matches!(result.stable, write::StableHow::Unstable));
    }
}
