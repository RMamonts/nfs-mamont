//! Implements parsing for [`mk_node::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::mk_node;

fn what(_src: &mut impl Read) -> Result<mk_node::What> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<mk_node::Args> {
    Ok(mk_node::Args { dir: file::handle(src)?, name: string(src)?, what: what(src)? })
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

        assert_eq!(result.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.name, "node");

        // TODO()
        // assert!(matches!(result.what, mk_node::What::Block(todo!(), todo!())));
    }
}
