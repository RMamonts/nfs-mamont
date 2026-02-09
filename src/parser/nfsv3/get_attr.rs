//! Implements parsing for [`get_attr::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::get_attr;

/// Parses the arguments for an NFSv3 `GETATTR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<get_attr::Args> {
    Ok(get_attr::Args { file: file::handle(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::args;

    #[test]
    fn test_get_attr() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08
        ];

        let args = args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(args.file.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }
}
