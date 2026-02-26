//! Implements parsing for [`mk_node::Args`] structure.

use crate::parser::nfsv3::create::new_attr;
use crate::parser::nfsv3::{file, MAX_FILENAME};
use crate::parser::primitive::{string_max_size, u32};
use crate::parser::{Error, Result};
use crate::vfs::file::Device;
use crate::vfs::mk_node;
use crate::vfs::mk_node::What;
use std::io::Read;

fn what(src: &mut impl Read) -> Result<mk_node::What> {
    match u32(src)? {
        3 => Ok(What::Block(new_attr(src)?, Device { major: u32(src)?, minor: u32(src)? })),
        4 => Ok(What::Char(new_attr(src)?, Device { major: u32(src)?, minor: u32(src)? })),
        6 => Ok(What::Socket(new_attr(src)?)),
        7 => Ok(What::Fifo(new_attr(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Parses the arguments for an NFSv3 `MKNOD` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<mk_node::Args> {
    Ok(mk_node::Args {
        dir: file::handle(src)?,
        name: string_max_size(src, MAX_FILENAME)?,
        what: what(src)?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::Error;
    use crate::vfs::mk_node;
    use crate::vfs::set_attr;

    #[rustfmt::skip]
    const EMPTY_NEW_ATTR: &[u8] = &[
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    #[rustfmt::skip]
    const ARGS_PREFIX_NODE: &[u8] = &[
        0x00, 0x00, 0x00, 0x08,
        0x01, 0x02, 0x03, 0x04,
        0x05, 0x06, 0x07, 0x08,
        0x00, 0x00, 0x00, 0x04,
        b'n', b'o', b'd', b'e',
    ];

    fn wrap_what_with_args(what_bytes: &[u8]) -> Vec<u8> {
        [ARGS_PREFIX_NODE, what_bytes].concat()
    }

    #[test]
    fn test_mknod_fifo() {
        let data = wrap_what_with_args(&[[0, 0, 0, 7].as_slice(), EMPTY_NEW_ATTR].concat());
        let result = super::args(&mut Cursor::new(data)).unwrap();

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.name, "node");
        assert!(matches!(
            result.what,
            mk_node::What::Fifo(set_attr::NewAttr {
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
    fn test_what_block() {
        let data = [
            [0, 0, 0, 3].as_slice(),
            EMPTY_NEW_ATTR,
            [0, 0, 0, 17].as_slice(),
            [0, 0, 0, 34].as_slice(),
        ]
        .concat();
        let result = super::what(&mut Cursor::new(data)).unwrap();

        assert!(matches!(
            result,
            mk_node::What::Block(
                set_attr::NewAttr {
                    mode: None,
                    uid: None,
                    gid: None,
                    size: None,
                    atime: set_attr::SetTime::DontChange,
                    mtime: set_attr::SetTime::DontChange,
                },
                crate::vfs::file::Device { major: 17, minor: 34 }
            )
        ));
    }

    #[test]
    fn test_what_char() {
        let data = [
            [0, 0, 0, 4].as_slice(),
            EMPTY_NEW_ATTR,
            [0, 0, 0, 170].as_slice(),
            [0, 0, 0, 187].as_slice(),
        ]
        .concat();
        let result = super::what(&mut Cursor::new(data)).unwrap();

        assert!(matches!(
            result,
            mk_node::What::Char(
                set_attr::NewAttr {
                    mode: None,
                    uid: None,
                    gid: None,
                    size: None,
                    atime: set_attr::SetTime::DontChange,
                    mtime: set_attr::SetTime::DontChange,
                },
                crate::vfs::file::Device { major: 170, minor: 187 }
            )
        ));
    }

    #[test]
    fn test_what_socket() {
        let data = [[0, 0, 0, 6].as_slice(), EMPTY_NEW_ATTR].concat();
        let result = super::what(&mut Cursor::new(data)).unwrap();

        assert!(matches!(
            result,
            mk_node::What::Socket(set_attr::NewAttr {
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
    fn test_what_invalid_discriminants() {
        for disc in [1_u32, 2, 5, 8] {
            let bytes = disc.to_be_bytes();
            assert!(matches!(super::what(&mut Cursor::new(bytes)), Err(Error::EnumDiscMismatch)));
        }
    }

    #[test]
    fn test_args_invalid_what_discriminant() {
        let data = wrap_what_with_args(&[0, 0, 0, 1]);

        assert!(matches!(super::args(&mut Cursor::new(data)), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_args_truncated_after_what() {
        let data = wrap_what_with_args(&[0, 0, 0, 7]);

        assert!(matches!(super::args(&mut Cursor::new(data)), Err(Error::IO(_))));
    }
}
