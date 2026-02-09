use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::{option, u32};
use crate::vfs::access;

pub fn result_ok(dest: &mut impl Write, arg: access::Success) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, attr))?;
    u32(dest, arg.access.0)
}

pub fn result_fail(dest: &mut impl Write, arg: access::Fail) -> io::Result<()> {
    option(dest, arg.object_attr, |attr, dest| file_attr(dest, attr))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::vfs::access::Mask;
    use crate::vfs::file;
    use crate::vfs::file::{Device, Time};

    #[test]
    fn test_res_ok() {
        #[rustfmt::skip]
        let expected = vec![
            0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x2D,
            0x00, 0x00, 0x00, 0x40,
            0x00, 0x00, 0x00, 0x80,
            0x00, 0x00, 0x08, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x0F,
            0x00, 0x00, 0x00, 0x4E,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x00, 0x10,
            0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x00, 0x40,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x02E,
        ];

        let mut buf = Cursor::new(vec![1u8; 92]);

        let test = access::Success {
            object_attr: Some(file::Attr {
                file_type: file::Type::Regular,
                mode: 45,
                nlink: 64,
                uid: 128,
                gid: 2048,
                size: 256,
                used: 0,
                device: Device { major: 15, minor: 78 },
                fs_id: 0,
                file_id: 0,
                atime: Time { seconds: 0, nanos: 512 },
                mtime: Time { seconds: 16, nanos: 512 },
                ctime: Time { seconds: 64, nanos: 0 },
            }),
            access: Mask(46),
        };
        result_ok(&mut buf, test).unwrap();
        assert_eq!(buf.into_inner(), expected);
    }
}
