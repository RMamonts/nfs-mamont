//! Implements parsing for [`lookup::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::{file, MAX_FILENAME};
use crate::parser::primitive::string_max_size;
use crate::parser::Result;
use crate::vfs::lookup;

/// Parses the arguments for an NFSv3 `LOOKUP` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<lookup::Args> {
    Ok(lookup::Args { parent: file::handle(src)?, name: string_max_size(src, MAX_FILENAME)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::args;

    #[test]
    fn test_lookup() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x04,
            b't', b'e', b's', b't',
        ];

        let args = args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(args.parent.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(args.name, "test");
    }

    #[test]
    fn test_lookup_unaligned_name() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x03,
            b'a', b'b', b'c',
        ];

        let result = super::args(&mut Cursor::new(DATA));
        assert!(result.is_err());
    }
}
