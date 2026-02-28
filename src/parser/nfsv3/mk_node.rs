//! Implements parsing for [`mk_node::Args`] structure.
use std::io::Read;

use crate::parser::nfsv3::create::new_attr;
use crate::parser::nfsv3::{file, MAX_FILENAME};
use crate::parser::primitive::{string_max_size, u32};
use crate::parser::{Error, Result};
use crate::vfs::file::Device;
use crate::vfs::mk_node;
use crate::vfs::mk_node::What;

fn what(src: &mut impl Read) -> Result<mk_node::What> {
    match u32(src)? {
        1 => Ok(What::Regular),
        2 => Ok(What::Directory),
        3 => Ok(What::Block(new_attr(src)?, Device { major: u32(src)?, minor: u32(src)? })),
        4 => Ok(What::Char(new_attr(src)?, Device { major: u32(src)?, minor: u32(src)? })),
        5 => Ok(What::SymbolicLink),
        6 => Ok(What::Socket(new_attr(src)?)),
        7 => Ok(What::Fifo(new_attr(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Parses the arguments for an NFSv3 `MKNOD` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<mk_node::Args> {
    Ok(mk_node::Args {
        object: crate::vfs::DirOpArgs {
            dir: file::handle(src)?,
            name: string_max_size(src, MAX_FILENAME)?,
        },
        what: what(src)?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn test_mknod_regular() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x04,
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x04,
            b'n', b'o', b'd', b'e', 0x00, 0x00, 0x00, 0x01,
        ];

        let result = super::args(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.object.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.object.name, "node");

        // TODO()
        // assert!(matches!(result.what, mk_node::What::Block(todo!(), todo!())));
    }
}
