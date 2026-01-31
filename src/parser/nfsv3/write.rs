//! Implements parsing for [`write::Args`] structure.

use crate::parser::nfsv3::file;
use crate::parser::primitive::{u32, u64};
use crate::parser::{Error, Result};
use crate::vfs::write;
use crate::vfs::write::StableHow;
use num_traits::FromPrimitive;
use std::io::Read;

fn stable_how(src: &mut impl Read) -> Result<write::StableHow> {
    StableHow::from_u32(u32(src)?).ok_or(Error::EnumDiscMismatch)
}

pub fn args(src: &mut impl Read) -> Result<write::ArgsPartial> {
    Ok(write::ArgsPartial {
        file: file::handle(src)?,
        offset: u64(src)?,
        stable: stable_how(src)?,
        size: u32(src)?,
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
        assert_eq!(result.size, 1024);
        assert!(matches!(result.stable, write::StableHow::Unstable));
    }
}
