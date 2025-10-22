use std::io::Read;

use crate::nfsv3::{
    createhow3, devicedata3, diropargs3, mknoddata3, nfs_fh3, nfstime3, sattr3, set_atime,
    set_mtime, specdata3, symlinkdata3, NFS3_CREATEVERFSIZE,
};
use crate::parser::to_parse::{
    parse_array, parse_option, parse_string_max_len, parse_u32, parse_u64, parse_u8,
    parse_vec_max_size,
};
use crate::parser::Error;

#[allow(dead_code)]
const MAX_FILENAME: usize = 255;
#[allow(dead_code)]
const MAX_FILEHANDLE: usize = 255;
#[allow(dead_code)]
const MAX_FILEPATH: usize = 255;

#[allow(dead_code)]
fn parse_specdata3(src: &mut impl Read) -> Result<specdata3, Error> {
    Ok(specdata3 { specdata1: parse_u32(src)?, specdata2: parse_u32(src)? })
}

fn parse_nfstime(src: &mut impl Read) -> Result<nfstime3, Error> {
    Ok(nfstime3 { seconds: parse_u32(src)?, nseconds: parse_u32(src)? })
}

#[allow(dead_code)]
fn parse_set_atime(src: &mut impl Read) -> Result<set_atime, Error> {
    let disc = parse_u32(src)?;
    match disc {
        0 => Ok(set_atime::DONT_CHANGE),
        1 => Ok(set_atime::SET_TO_SERVER_TIME),
        2 => Ok(set_atime::SET_TO_CLIENT_TIME(parse_nfstime(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
fn parse_set_mtime(src: &mut impl Read) -> Result<set_mtime, Error> {
    let disc = parse_u32(src)?;
    match disc {
        0 => Ok(set_mtime::DONT_CHANGE),
        1 => Ok(set_mtime::SET_TO_SERVER_TIME),
        2 => Ok(set_mtime::SET_TO_CLIENT_TIME(parse_nfstime(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
fn parse_sattr3(src: &mut impl Read) -> Result<sattr3, Error> {
    Ok(sattr3 {
        mode: parse_option(src, |s| parse_u32(s))?,
        uid: parse_option(src, |s| parse_u32(s))?,
        gid: parse_option(src, |s| parse_u32(s))?,
        size: parse_option(src, |s| parse_u64(s))?,
        atime: parse_set_atime(src)?,
        mtime: parse_set_mtime(src)?,
    })
}

#[allow(dead_code)]
fn parse_nfs_fh3(src: &mut impl Read) -> Result<nfs_fh3, Error> {
    Ok(nfs_fh3 { data: parse_vec_max_size::<MAX_FILEHANDLE, u8>(src, |s| parse_u8(s))? })
}

#[allow(dead_code)]
fn parse_diropargs3(src: &mut impl Read) -> Result<diropargs3, Error> {
    Ok(diropargs3 { dir: parse_nfs_fh3(src)?, name: parse_string_max_len::<MAX_FILEPATH>(src)? })
}

#[allow(dead_code)]
fn parse_createhow3(src: &mut impl Read) -> Result<createhow3, Error> {
    let disc = parse_u32(src)?;
    match disc {
        0 => Ok(createhow3::UNCHECKED(parse_sattr3(src)?)),
        1 => Ok(createhow3::UNCHECKED(parse_sattr3(src)?)),
        2 => {
            Ok(createhow3::EXCLUSIVE(parse_array::<NFS3_CREATEVERFSIZE, u8>(src, |s| parse_u8(s))?))
        }
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[allow(dead_code)]
fn parse_symlinkdata3(src: &mut impl Read) -> Result<symlinkdata3, Error> {
    Ok(symlinkdata3 {
        symlink_attributes: parse_sattr3(src)?,
        symlink_data: parse_string_max_len::<MAX_FILEPATH>(src)?,
    })
}

#[allow(dead_code)]
fn parse_devicedata3(src: &mut impl Read) -> Result<devicedata3, Error> {
    Ok(devicedata3 { dev_attributes: parse_sattr3(src)?, spec: parse_specdata3(src)? })
}

#[allow(dead_code)]
fn parse_mknoddata3(src: &mut impl Read) -> Result<mknoddata3, Error> {
    let disc = parse_u32(src)?;
    match disc {
        1 => Ok(mknoddata3::NF3REG),
        2 => Ok(mknoddata3::NF3DIR),
        3 => Ok(mknoddata3::NF3BLK(parse_devicedata3(src)?)),
        4 => Ok(mknoddata3::NF3CHR(parse_devicedata3(src)?)),
        5 => Ok(mknoddata3::NF3LNK),
        6 => Ok(mknoddata3::NF3SOCK(parse_sattr3(src)?)),
        7 => Ok(mknoddata3::NF3FIFO(parse_sattr3(src)?)),
        _ => Err(Error::EnumDiscMismatch),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::nfsv3::ftype3;
    use crate::parser::to_parse::parse_c_enum;

    use super::*;

    #[test]
    fn test_parse_specdata3_success() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_specdata3(&mut src).unwrap();
        assert_eq!(result.specdata1, 1);
        assert_eq!(result.specdata2, 2);
    }

    #[test]
    fn test_parse_specdata3_error() {
        let data = [0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);

        assert!(matches!(parse_specdata3(&mut src), Err(Error::IO)));
    }

    #[test]
    fn test_parse_nfstime_success() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);

        let result = parse_nfstime(&mut src).unwrap();
        assert_eq!(result.seconds, 1);
        assert_eq!(result.nseconds, 2);
    }

    #[test]
    fn test_parse_nfstime_error() {
        let data = [0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);

        assert!(matches!(parse_nfstime(&mut src), Err(Error::IO)));
    }

    #[test]
    fn test_parse_set_atime_all_cases() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = parse_set_atime(&mut src).unwrap();
        assert!(matches!(result, set_atime::DONT_CHANGE));

        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = parse_set_atime(&mut src).unwrap();
        assert!(matches!(result, set_atime::SET_TO_SERVER_TIME));

        let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_set_atime(&mut src).unwrap();
        match result {
            set_atime::SET_TO_CLIENT_TIME(nfstime) => {
                assert_eq!(nfstime.seconds, 1);
                assert_eq!(nfstime.nseconds, 2);
            }
            _ => panic!("Expected SET_TO_CLIENT_TIME"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);
        assert!(matches!(parse_set_atime(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_parse_set_mtime_all_variants() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = parse_set_mtime(&mut src).unwrap();
        assert!(matches!(result, set_mtime::DONT_CHANGE));

        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = parse_set_mtime(&mut src).unwrap();
        assert!(matches!(result, set_mtime::SET_TO_SERVER_TIME));

        let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_set_mtime(&mut src).unwrap();
        match result {
            set_mtime::SET_TO_CLIENT_TIME(nfstime) => {
                assert_eq!(nfstime.seconds, 1);
                assert_eq!(nfstime.nseconds, 2);
            }
            _ => panic!("Expected SET_TO_CLIENT_TIME"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);
        assert!(matches!(parse_set_mtime(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_parse_sattr3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_sattr3(&mut src).unwrap();
        assert!(result.mode.is_none());
        assert_eq!(result.uid, Some(1));
        assert!(result.gid.is_none());
        assert_eq!(result.size, Some(1));
        assert!(matches!(result.atime, set_atime::DONT_CHANGE));
        assert!(matches!(result.mtime, set_mtime::SET_TO_SERVER_TIME));
    }

    #[test]
    fn test_parse_nfs_fh3_success() {
        let data = [0x00, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00];
        let mut src = Cursor::new(&data);

        let result = parse_nfs_fh3(&mut src).unwrap();
        assert_eq!(result.data, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_parse_nfs_fh3_exceeds_max_size() {
        let mut data = vec![0xFF, 0xFF, 0xFF, 0xFF];
        data.extend(vec![0x00; MAX_FILEHANDLE + 1]);

        let mut src = Cursor::new(&data);
        assert!(matches!(parse_nfs_fh3(&mut src), Err(Error::VecTooLong)));
    }

    #[test]
    fn test_parse_diropargs3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, b'a', b'b',
            b'c', 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_diropargs3(&mut src).unwrap();
        assert_eq!(result.dir.data, vec![0x01, 0x02]);
        assert_eq!(result.name, "abc");
    }

    #[test]
    fn test_parse_createhow3_unchecked() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_createhow3(&mut src).unwrap();
        assert!(matches!(result, createhow3::UNCHECKED(_)));

        let data = [
            0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
            0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_createhow3(&mut src).unwrap();
        match result {
            createhow3::EXCLUSIVE(verifier) => {
                assert_eq!(verifier.len(), NFS3_CREATEVERFSIZE);
                assert_eq!(verifier[0], 0x01);
            }
            _ => panic!("Expected EXCLUSIVE"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);

        assert!(matches!(parse_createhow3(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_parse_symlinkdata3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
            b'h', b'e', b'l', b'l', b'o', 0x00, 0x00, 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_symlinkdata3(&mut src).unwrap();
        assert_eq!(result.symlink_data, "hello");
    }

    #[test]
    fn test_parse_devicedata3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x02,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_devicedata3(&mut src).unwrap();
        assert_eq!(result.spec.specdata1, 1);
        assert_eq!(result.spec.specdata2, 2);
    }

    #[test]
    fn test_parse_mknoddata3_all_variants() {
        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3REG));

        let data = [0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3DIR));

        let data = [
            0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
        ];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3BLK(_)));

        let data = [0x00, 0x00, 0x00, 0x05];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3LNK));

        let data = [0x00, 0x00, 0x00, 0x08];
        let mut src = Cursor::new(&data);
        assert!(matches!(parse_mknoddata3(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_c_enum() {
        let data = [0x00, 0x00, 0x00, 0x06];
        let mut src = Cursor::new(&data);
        let result = parse_c_enum(&mut src).unwrap();
        assert!(matches!(result, ftype3::NF3SOCK));

        let data = [0x00, 0x00, 0x00, 0x08];
        let mut src = Cursor::new(&data);
        let result = parse_c_enum::<ftype3>(&mut src);
        assert!(matches!(result, Err(Error::EnumDiscMismatch)));
    }
}
