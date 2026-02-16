//! Implements [`crate::vfs::file`] structures parsing

use std::io::Read;

use crate::parser::primitive::{array, option, string_max_size, u32, u32_as_usize, u64};
use crate::parser::{Error, Result};
use crate::vfs;
use crate::vfs::file::{Name, Path};
use crate::vfs::{file, MAX_PATH_LEN};

/// Parses a [`file::Handle`] from the provided `Read` source.
pub fn handle(src: &mut impl Read) -> Result<file::Handle> {
    if u32_as_usize(src)? != file::HANDLE_SIZE {
        return Err(Error::BadFileHandle);
    }
    let array = array::<{ file::HANDLE_SIZE }>(src)?;
    Ok(file::Handle(array))
}

/// Parses a [`file::Type`] from the provided `Read` source.
pub fn r#type(src: &mut impl Read) -> Result<file::Type> {
    use file::Type::*;

    Ok(match u32(src)? {
        1 => Regular,
        2 => Directory,
        3 => BlockDevice,
        4 => CharacterDevice,
        5 => Symlink,
        6 => Socket,
        7 => Fifo,
        _ => return Err(Error::EnumDiscMismatch),
    })
}

/// Parses a [`file::Attr`] structure from the provided `Read` source.
pub fn attr(src: &mut impl Read) -> Result<file::Attr> {
    Ok(file::Attr {
        file_type: r#type(src)?,
        mode: u32(src)?,
        nlink: u32(src)?,
        uid: u32(src)?,
        gid: u32(src)?,
        size: u64(src)?,
        used: u64(src)?,
        device: option(src, |s| device(s))?,
        fs_id: u64(src)?,
        file_id: u64(src)?,
        atime: time(src)?,
        mtime: time(src)?,
        ctime: time(src)?,
    })
}

/// Parses a [`file::Time`] structure from the provided `Read` source.
pub fn time(src: &mut impl Read) -> Result<file::Time> {
    Ok(file::Time { seconds: u32(src)?, nanos: u32(src)? })
}

/// Parses a [`file::Device`] structure from the provided `Read` source.
pub fn device(src: &mut impl Read) -> Result<file::Device> {
    Ok(file::Device { major: u32(src)?, minor: u32(src)? })
}

/// Parses a [`file::WccAttr`] structure from the provided `Read` source.
pub fn wcc_attr(src: &mut impl Read) -> Result<file::WccAttr> {
    Ok(file::WccAttr { size: u64(src)?, mtime: time(src)?, ctime: time(src)? })
}

/// Parses a [`file::Name`] structure from the provided `Read` source.
pub fn file_name(src: &mut impl Read) -> Result<file::Name> {
    Name::new(string_max_size(src, vfs::MAX_NAME_LEN)?).map_err(|_| Error::MaxELemLimit)
}

/// Parses a [`file::Path`] structure from the provided `Read` source.
pub fn file_path(src: &mut impl Read) -> Result<file::Path> {
    Path::new(string_max_size(src, MAX_PATH_LEN)?).map_err(|_| Error::MaxELemLimit)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::Error;
    use crate::vfs::file;

    use super::device;

    #[test]
    fn test_parse_device_success() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02
        ];

        let result = device(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.major, 1);
        assert_eq!(result.minor, 2);
    }

    #[test]
    fn test_device_error() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[0x00, 0x00, 0x01];
        let mut src = Cursor::new(DATA);

        assert!(matches!(device(&mut src), Err(Error::IO(_))));
    }

    #[test]
    fn test_nfstime_success() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02
        ];

        let result = super::time(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.seconds, 1);
        assert_eq!(result.nanos, 2);
    }

    #[test]
    fn test_nfstime_error() {
        const DATA: &[u8] = &[0x00, 0x00, 0x01];

        assert!(matches!(super::time(&mut Cursor::new(&DATA)), Err(Error::IO(_))));
    }

    #[test]
    fn test_nfs_fh3_success() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x00,
            0x00, 0x00, 0x00, 0x00
        ];

        let result = super::handle(&mut Cursor::new(DATA)).unwrap();

        assert_eq!(result.0, [0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_nfs_fh3_badfh() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00,
            0x00, 0x00, 0x00, 0x00
        ];

        let result = super::handle(&mut Cursor::new(DATA));

        assert!(matches!(result, Err(Error::BadFileHandle)));
    }

    #[test]
    fn test_type_regular() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x01];

        let result = super::r#type(&mut Cursor::new(DATA)).unwrap();
        assert!(matches!(result, file::Type::Regular));
    }

    #[test]
    fn test_type_dir() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x02];

        let result = super::r#type(&mut Cursor::new(DATA)).unwrap();
        assert!(matches!(result, file::Type::Directory));
    }

    #[test]
    fn test_type_block() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x02,
        ];

        let result = super::r#type(&mut Cursor::new(DATA)).unwrap();
        assert!(matches!(result, file::Type::BlockDevice));
    }

    #[test]
    fn test_type_symlink() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x05];

        let result = super::r#type(&mut Cursor::new(DATA)).unwrap();
        assert!(matches!(result, file::Type::Symlink));
    }

    #[test]
    fn test_type_failure() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x08];

        assert!(matches!(super::r#type(&mut Cursor::new(DATA)), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_file_path_success() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x04, b'f', b'i', b'l', b'e'];
        let file = file::Path::new("file".to_string()).unwrap();
        assert_eq!(super::file_path(&mut Cursor::new(DATA)).unwrap(), file);
    }

    #[test]
    fn test_file_path_padding_error() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x02, b'f', b'i', 0x00];

        assert!(matches!(super::file_path(&mut Cursor::new(DATA)), Err(Error::IncorrectPadding)));
    }

    #[test]
    fn test_file_name_success() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x04, b'f', b'i', b'l', b'e'];
        let file = file::Name::new("file".to_string()).unwrap();
        assert_eq!(super::file_name(&mut Cursor::new(DATA)).unwrap(), file);
    }
}
