//! Shared XDR serializers for common NFSv3 data structures.

use std::io;
use std::io::{ErrorKind, Result, Write};

use crate::serializer::{array, option, string_max_size, u32, u64, usize_as_u32, variant};
use crate::vfs;
use crate::vfs::{file, MAX_PATH_LEN};

const MAX_FILEHANDLE: usize = 8;

/// Serializes [`vfs::file::Time`] into XDR `nfstime3`.
pub fn nfs_time(dest: &mut impl Write, arg: file::Time) -> Result<()> {
    u32(dest, arg.seconds).and_then(|_| u32(dest, arg.nanos))
}

/// Serializes [`vfs::file::Handle`] into XDR `nfs_fh3`.
pub fn file_handle(dest: &mut impl Write, fh: file::Handle) -> Result<()> {
    usize_as_u32(dest, MAX_FILEHANDLE).and_then(|_| array::<MAX_FILEHANDLE>(dest, fh.0))
}

/// Serializes [`vfs::Error`] as an XDR enum discriminant (NFS status).
pub fn error(dest: &mut impl Write, stat: vfs::Error) -> Result<()> {
    variant(dest, stat)
}

/// Serializes [file::Type] as the XDR `ftype3` enum discriminant.
pub fn file_type(dest: &mut impl Write, file_type: file::Type) -> io::Result<()> {
    variant::<file::Type>(dest, file_type)
}

/// Serializes [`file::Attr`] as XDR `fattr3` (file attributes).
pub fn file_attr(dest: &mut impl Write, attr: &file::Attr) -> io::Result<()> {
    file_type(dest, attr.file_type)?;
    u32(dest, attr.mode)?;
    u32(dest, attr.nlink)?;
    u32(dest, attr.uid)?;
    u32(dest, attr.gid)?;
    u64(dest, attr.size)?;
    u64(dest, attr.used)?;
    u32(dest, attr.device.major)?;
    u32(dest, attr.device.minor)?;
    u64(dest, attr.fs_id)?;
    u64(dest, attr.file_id)?;
    nfs_time(dest, attr.atime)?;
    nfs_time(dest, attr.mtime)?;
    nfs_time(dest, attr.ctime)
}

/// Serializes [`file::WccAttr`] as XDR `wcc_attr` (weak cache consistency).
pub fn wcc_attr(dest: &mut impl Write, wcc: file::WccAttr) -> io::Result<()> {
    u64(dest, wcc.size)?;
    nfs_time(dest, wcc.mtime)?;
    nfs_time(dest, wcc.ctime)
}

/// Serializes [`vfs::WccData`] as XDR `wcc_data` (before/after attributes).
pub fn wcc_data(dest: &mut impl Write, wcc: vfs::WccData) -> io::Result<()> {
    option(dest, wcc.before, |attr, dest| wcc_attr(dest, attr))?;
    option(dest, wcc.after, |attr, dest| file_attr(dest, &attr))
}

/// Serializes [`file::Name`] as XDR `filename3` (bounded string).
pub fn file_name(dest: &mut impl Write, file_name: file::Name) -> io::Result<()> {
    string_max_size(dest, file_name.into_inner(), vfs::MAX_NAME_LEN)
}

/// Serializes [`file::Path`] as XDR `path` (bounded string).
pub fn file_path(dest: &mut impl Write, file_name: file::Path) -> io::Result<()> {
    string_max_size(
        dest,
        file_name
            .into_inner()
            .into_os_string()
            .into_string()
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid path"))?,
        MAX_PATH_LEN,
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::vfs::file;
    use crate::vfs::file::Time;

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

    #[test]
    fn test_wcc_attr_success() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x52,
            0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x01, 0x01,
            0x00, 0x00, 0x00, 0xA0, 0x00, 0x00, 0x05, 0x23,
        ];

        let expected = file::WccAttr {
            size: 82,
            mtime: Time { seconds: 15, nanos: 257 },
            ctime: Time { seconds: 160, nanos: 1315 },
        };

        assert_eq!(super::wcc_attr(&mut Cursor::new(DATA)).unwrap(), expected);
    }
}
