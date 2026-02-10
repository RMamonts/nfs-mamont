//! Implements parsing for [`mk_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::create::new_attr;
use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::mk_dir;

/// Parses the arguments for an NFSv3 `MKDIR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<mk_dir::Args> {
    Ok(mk_dir::Args { dir: file::handle(src)?, name: file_name(src)?, attr: new_attr(src)? })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::vfs::set_attr;

    #[test]
    fn test_mkdir() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x04,
            b'd', b'i', b'r', b'1', 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x25, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.name.0, "dir1");
        assert!(matches!(
            result.attr,
            set_attr::NewAttr {
                mode: Some(0x25),
                uid: None,
                gid: None,
                size: None,
                atime: set_attr::SetTime::ToServer,
                mtime: set_attr::SetTime::ToServer,
            }
        ));
    }
}
