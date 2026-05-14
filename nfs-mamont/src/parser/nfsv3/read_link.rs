//! Implements parsing for [`read_link::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::read_link;

/// Parses the arguments for an NFSv3 `READLINK` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<read_link::Args> {
    Ok(read_link::Args { file: file::handle(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn test_readlink() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.file.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
    }
}
