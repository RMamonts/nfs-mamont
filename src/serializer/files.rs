//! Shared XDR serializers for common NFSv3 data structures.
use std::io;
use std::io::{ErrorKind, Write};

use crate::nfsv3::NFS3_FHSIZE;
use crate::serializer::{array, option, string_max_size, u32, u64, usize_as_u32, variant};
use crate::vfs;
use crate::vfs::{file, MAX_PATH_LEN};

/// Serializes [`vfs::file::Time`] into XDR `nfstime3`.
pub fn nfs_time(dest: &mut impl Write, arg: file::Time) -> io::Result<()> {
    u32(dest, arg.seconds).and_then(|_| u32(dest, arg.nanos))
}

/// Serializes [`vfs::file::Handle`] into XDR `nfs_fh3`.
pub fn file_handle(dest: &mut impl Write, fh: file::Handle) -> io::Result<()> {
    usize_as_u32(dest, NFS3_FHSIZE).and_then(|_| array(dest, fh.0))
}

/// Serializes [`vfs::Error`] as an XDR enum discriminant (NFS status).
pub fn error(dest: &mut impl Write, stat: vfs::Error) -> io::Result<()> {
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

    #[test]
    fn test_nfstime_success() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x02,
            0x01
        ];

        let mut buffer = Cursor::new([1u8; 9]);

        let time = file::Time { seconds: 1, nanos: 2 };

        nfs_time(&mut buffer, time).unwrap();

        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_nfs_fh3_success() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x08,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x01
        ];

        let mut buffer = Cursor::new([1u8; 13]);

        let handle = file::Handle([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);

        file_handle(&mut buffer, handle).unwrap();

        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_type_regular() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x01, 0x01];

        let mut buffer = Cursor::new([1u8; 5]);

        file_type(&mut buffer, file::Type::Regular).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_type_dir() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x02, 0x01];

        let mut buffer = Cursor::new([1u8; 5]);

        file_type(&mut buffer, file::Type::Directory).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_type_symlink() {
        const DATA: &[u8] = &[0x00, 0x00, 0x00, 0x05, 0x01];

        let mut buffer = Cursor::new([1u8; 5]);

        file_type(&mut buffer, file::Type::Symlink).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_file_path_with_padding() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x05,
            b'd', b'i', b'r', b'/',
            b'0', 0x00, 0x00, 0x00,
            0x01
        ];

        let mut buffer = Cursor::new([1u8; 13]);
        let file = file::Path::new("dir/0".to_string()).unwrap();
        file_path(&mut buffer, file).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_file_path_without_padding() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x04,
            b'/', b'd', b'/', b'e',
            0x01
        ];

        let mut buffer = Cursor::new([1u8; 9]);
        let file = file::Path::new("/d/e".to_string()).unwrap();
        file_path(&mut buffer, file).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_file_name_without_padding() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x04,
            b'f', b'i', b'l', b'e',
            0x01
        ];

        let file = file::Name::new("file".to_string()).unwrap();

        let mut buffer = Cursor::new([1u8; 9]);
        file_name(&mut buffer, file).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_file_name_with_padding() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x05,
            b'f', b'i', b'l', b'e',
            b'0', 0x00, 0x00, 0x00,
            0x01
        ];

        let mut buffer = Cursor::new([1u8; 13]);
        let file = file::Name::new("file0".to_string()).unwrap();
        file_name(&mut buffer, file).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }

    #[test]
    fn test_wcc_attr_success() {
        #[rustfmt::skip]
        const DATA: &[u8] = &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x52,
            0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x01, 0x01,
            0x00, 0x00, 0x00, 0xA0, 0x00, 0x00, 0x05, 0x23,
        ];

        let attr = file::WccAttr {
            size: 82,
            mtime: file::Time { seconds: 15, nanos: 257 },
            ctime: file::Time { seconds: 160, nanos: 1315 },
        };

        let mut buffer = Cursor::new([1u8; 24]);
        wcc_attr(&mut buffer, attr).unwrap();
        assert_eq!(buffer.into_inner(), DATA);
    }
}
