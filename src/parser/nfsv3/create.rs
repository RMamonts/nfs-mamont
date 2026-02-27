//! Implements parsing for [`create::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::{file, MAX_FILENAME};
use crate::parser::primitive::{array, option, string_max_size, u32, u64};
use crate::parser::{Error, Result};
use crate::vfs::create;
use crate::vfs::create::{Verifier, VERIFY_LEN};
use crate::vfs::file::Time;
use crate::vfs::set_attr::{NewAttr, SetTime};

/// Parses a [`NewAttr`] structure from the provided `Read` source.
pub fn new_attr(src: &mut impl Read) -> Result<NewAttr> {
    Ok(NewAttr {
        mode: option(src, |s| u32(s))?,
        uid: option(src, |s| u32(s))?,
        gid: option(src, |s| u32(s))?,
        size: option(src, |s| u64(s))?,
        atime: set_time(src)?,
        mtime: set_time(src)?,
    })
}

/// Parses a [`SetTime`] enum from the provided `Read` source.
#[allow(dead_code)]
pub fn set_time(src: &mut impl Read) -> Result<SetTime> {
    match u32(src)? {
        0 => Ok(SetTime::DontChange),
        1 => Ok(SetTime::ToServer),
        2 => Ok(SetTime::ToClient(nfs_time(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Parses an NFS time structure from the provided `Read` source.
pub fn nfs_time(src: &mut impl Read) -> Result<Time> {
    Ok(Time { seconds: u32(src)?, nanos: u32(src)? })
}

/// Parses a [`create::How`] enum from the provided `Read` source.
pub fn how(src: &mut impl Read) -> Result<create::How> {
    match u32(src)? {
        0 => Ok(create::How::Unchecked(new_attr(src)?)),
        1 => Ok(create::How::Guarded(new_attr(src)?)),
        2 => Ok(create::How::Exclusive(Verifier(array::<{ VERIFY_LEN }>(src)?))),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Parses the arguments for an NFSv3 `CREATE` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<create::Args> {
    Ok(create::Args {
        object: crate::vfs::DirOpArgs {
            dir: file::handle(src)?,
            name: string_max_size(src, MAX_FILENAME)?,
        },
        how: how(src)?,
    })
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

        assert_eq!(result.object.dir.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(result.object.name, "file");
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
