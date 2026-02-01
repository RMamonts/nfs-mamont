//! Implements parsing for [`write::Args`] structure.

use crate::parser::nfsv3::file;
use crate::parser::primitive::{u32, u64, variant};
use crate::parser::Result;
use crate::vfs::write;
use crate::vfs::write::StableHow;

use std::io::Read;

fn stable_how(src: &mut impl Read) -> Result<write::StableHow> {
    variant::<StableHow>(src)
}

pub fn args(src: &mut impl Read) -> Result<write::ArgsPartial> {
    Ok(write::ArgsPartial {
        file: file::handle(src)?,
        offset: u64(src)?,
        size: u32(src)?,
        stable: stable_how(src)?,
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
            0x00, 0x00, 0x00, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.file.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.offset, 65536);
        assert_eq!(result.size, 1024);
        assert!(matches!(result.stable, write::StableHow::Unstable));
    }
}
