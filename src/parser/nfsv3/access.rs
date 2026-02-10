//! Implements parsing for [`access::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::u32;
use crate::parser::Result;
use crate::vfs::access;

/// Parses the arguments for an NFSv3 `ACCESS` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<access::Args> {
    Ok(access::Args { file: file::handle(src)?, mask: access::Mask(u32(src)?) })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::args;

    #[test]
    fn test_access() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x1F,
        ];

        let args = args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(args.file.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(args.mask.0, 0x1F);
    }
}
