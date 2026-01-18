//! Implements parsing for [`set_attr::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::set_attr;

pub fn new_attr(_src: &mut impl Read) -> Result<set_attr::NewAttr> {
    todo!()
}

pub fn guard(_src: &mut impl Read) -> Result<Option<set_attr::Guard>> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<set_attr::Args> {
    Ok(set_attr::Args { file: file::handle(src)?, new_attr: new_attr(src)?, guard: guard(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::Error;
    use crate::vfs::file;
    use crate::vfs::set_attr;

    use super::args;

    #[test]
    fn test_set_attr() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x02, 0x00, 0x00, 0x00, 0x00,
        ];

        let args = args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(args.file.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(args.new_attr.mode, Some(256));
        assert_eq!(args.new_attr.uid, Some(1));
        assert_eq!(args.new_attr.gid, Some(2));
        assert_eq!(args.new_attr.size, Some(16));
        assert!(matches!(
            args.new_attr.atime,
            set_attr::SetTime::ToClient(file::Time { seconds: 1, nanos: 0 })
        ));
        assert!(matches!(args.new_attr.mtime, set_attr::SetTime::ToServer));
        assert!(matches!(
            args.guard,
            Some(set_attr::Guard { ctime: file::Time { seconds: 2, nanos: 0 } })
        ));
    }

    #[test]
    fn test_set_attr_insufficient_data() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x01, 0xA4,
        ];

        let result = super::args(&mut Cursor::new(DATA));
        assert!(matches!(result, Err(Error::IO(_))));
    }
}
