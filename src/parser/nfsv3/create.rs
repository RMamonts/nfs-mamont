//! Implements parsing for [`create::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::create;

pub fn how(_src: &mut impl Read) -> Result<create::How> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<create::Args> {
    Ok(create::Args { dir: file::handle(src)?, name: string(src)?, how: how(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::Error;
    use crate::vfs::create;
    use crate::vfs::set_attr;

    #[test]
    fn test_create() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x04,
            b'f', b'i', b'l', b'e', 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.name, "file");
        assert!(matches!(
            result.how,
            create::How::Unchecked(set_attr::NewAttr {
                mode: None,
                uid: None,
                gid: None,
                size: None,
                atime: set_attr::SetTime::DontChange,
                mtime: set_attr::SetTime::DontChange,
            })
        ));
    }

    #[test]
    fn test_create_unaligned_after_name() {
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x00, 0x00,
            0x00, 0x02, b'a', b'b', 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
            0x00, 0x02,
        ];

        assert!(super::args(&mut Cursor::new(DATA)).is_err());
    }

    #[test]
    fn test_how_unchecked() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let result = super::how(&mut Cursor::new(&DATA)).unwrap();
        assert!(matches!(result, create::How::Unchecked(_)));
    }

    #[test]
    fn test_how_exclusive() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
            0x0D, 0x0E, 0x0F, 0x10,
        ];

        let result = super::how(&mut Cursor::new(&DATA)).unwrap();
        assert!(matches!(result, create::How::Exclusive(_)));
    }

    #[test]
    fn test_how_failure() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x03];

        assert!(matches!(super::how(&mut Cursor::new(DATA)), Err(Error::EnumDiscMismatch)));
    }
}
