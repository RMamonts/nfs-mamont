//! Implements parsing for [`read::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::{u32, u64};
use crate::parser::Result;
use crate::vfs::read;

/// Parses the arguments for an NFSv3 `READ` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<read::Args> {
    Ok(read::Args { file: file::handle(src)?, offset: u64(src)?, count: u32(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::Error;

    #[test]
    fn test_read() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.file.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.offset, 65536);
        assert_eq!(result.count, 1024);
    }

    #[test]
    fn test_read_insufficient_data() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07
        ];

        let result = super::args(&mut Cursor::new(DATA));
        assert!(matches!(result, Err(Error::UnexpectedEof)));
    }
}
