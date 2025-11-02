#![cfg(test)]

use std::io::Cursor;

use crate::nfsv3::{createhow3, ftype3, mknoddata3, set_atime, set_mtime, NFS3_CREATEVERFSIZE};
use crate::parser::nfsv3::*;
use crate::parser::primitive::variant;
use crate::parser::Error;

#[test]
fn test_parse_specdata3_success() {
    let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
    let mut src = Cursor::new(&data);
    let result = specdata3(&mut src).unwrap();
    assert_eq!(result.specdata1, 1);
    assert_eq!(result.specdata2, 2);
}

#[test]
fn test_specdata3_error() {
    let data = [0x00, 0x00, 0x01];
    let mut src = Cursor::new(&data);

    assert!(matches!(specdata3(&mut src), Err(Error::IO(_))));
}

#[test]
fn test_nfstime_success() {
    let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
    let mut src = Cursor::new(&data);

    let result = nfstime(&mut src).unwrap();
    assert_eq!(result.seconds, 1);
    assert_eq!(result.nseconds, 2);
}

#[test]
fn test_nfstime_error() {
    let data = [0x00, 0x00, 0x01];
    let mut src = Cursor::new(&data);

    assert!(matches!(nfstime(&mut src), Err(Error::IO(_))));
}

#[test]
fn test_set_atime_all_cases() {
    let data = [0x00, 0x00, 0x00, 0x00];
    let mut src = Cursor::new(&data);
    let result = set_atime(&mut src).unwrap();
    assert!(matches!(result, set_atime::DONT_CHANGE));

    let data = [0x00, 0x00, 0x00, 0x01];
    let mut src = Cursor::new(&data);
    let result = set_atime(&mut src).unwrap();
    assert!(matches!(result, set_atime::SET_TO_SERVER_TIME));

    let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
    let mut src = Cursor::new(&data);
    let result = set_atime(&mut src).unwrap();
    match result {
        set_atime::SET_TO_CLIENT_TIME(nfstime) => {
            assert_eq!(nfstime.seconds, 1);
            assert_eq!(nfstime.nseconds, 2);
        }
        _ => panic!("Expected SET_TO_CLIENT_TIME"),
    }

    let data = [0x00, 0x00, 0x00, 0x03];
    let mut src = Cursor::new(&data);
    assert!(matches!(set_atime(&mut src), Err(Error::EnumDiscMismatch)));
}

#[test]
fn test_set_mtime_all_variants() {
    let data = [0x00, 0x00, 0x00, 0x00];
    let mut src = Cursor::new(&data);
    let result = set_mtime(&mut src).unwrap();
    assert!(matches!(result, set_mtime::DONT_CHANGE));

    let data = [0x00, 0x00, 0x00, 0x01];
    let mut src = Cursor::new(&data);
    let result = set_mtime(&mut src).unwrap();
    assert!(matches!(result, set_mtime::SET_TO_SERVER_TIME));

    let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
    let mut src = Cursor::new(&data);
    let result = set_mtime(&mut src).unwrap();
    match result {
        set_mtime::SET_TO_CLIENT_TIME(nfstime) => {
            assert_eq!(nfstime.seconds, 1);
            assert_eq!(nfstime.nseconds, 2);
        }
        _ => panic!("Expected SET_TO_CLIENT_TIME"),
    }

    let data = [0x00, 0x00, 0x00, 0x03];
    let mut src = Cursor::new(&data);
    assert!(matches!(set_mtime(&mut src), Err(Error::EnumDiscMismatch)));
}

#[test]
fn test_sattr3_success() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ];
    let mut src = Cursor::new(&data);

    let result = sattr3(&mut src).unwrap();
    assert!(result.mode.is_none());
    assert_eq!(result.uid, Some(1));
    assert!(result.gid.is_none());
    assert_eq!(result.size, Some(1));
    assert!(matches!(result.atime, set_atime::DONT_CHANGE));
    assert!(matches!(result.mtime, set_mtime::SET_TO_SERVER_TIME));
}

#[test]
fn test_nfs_fh3_success() {
    let data = [0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut src = Cursor::new(&data);
    let result = nfs_fh3(&mut src).unwrap();
    assert_eq!(result.data, [0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]);
}

#[test]
fn test_nfs_fh3_badfh() {
    let data = [0x00, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut src = Cursor::new(&data);
    let result = nfs_fh3(&mut src);
    assert!(matches!(result, Err(Error::BadFileHandle)));
}

#[test]
fn test_diropargs3_success() {
    let data = [
        0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x03, b'a', b'b', b'c', 0x00,
    ];
    let mut src = Cursor::new(&data);

    let result = diropargs3(&mut src).unwrap();
    assert_eq!(result.dir.data, [0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(result.name, "abc");
}

#[test]
fn test_createhow3_unchecked() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut src = Cursor::new(&data);

    let result = createhow3(&mut src).unwrap();
    assert!(matches!(result, createhow3::UNCHECKED(_)));

    let data = [
        0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        0x0C, 0x0D, 0x0E, 0x0F, 0x10,
    ];
    let mut src = Cursor::new(&data);

    let result = createhow3(&mut src).unwrap();
    match result {
        createhow3::EXCLUSIVE(verifier) => {
            assert_eq!(verifier.len(), NFS3_CREATEVERFSIZE);
            assert_eq!(verifier[0], 0x01);
        }
        _ => panic!("Expected EXCLUSIVE"),
    }

    let data = [0x00, 0x00, 0x00, 0x03];
    let mut src = Cursor::new(&data);

    assert!(matches!(createhow3(&mut src), Err(Error::EnumDiscMismatch)));
}

#[test]
fn test_symlinkdata3_success() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, b'h', b'e',
        b'l', b'l', b'o', 0x00, 0x00, 0x00,
    ];
    let mut src = Cursor::new(&data);

    let result = symlinkdata3(&mut src).unwrap();
    assert_eq!(result.symlink_data, "hello");
}

#[test]
fn test_devicedata3_success() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x02,
    ];
    let mut src = Cursor::new(&data);

    let result = devicedata3(&mut src).unwrap();
    assert_eq!(result.spec.specdata1, 1);
    assert_eq!(result.spec.specdata2, 2);
}

#[test]
fn test_mknoddata3_all_variants() {
    let data = [0x00, 0x00, 0x00, 0x01];
    let mut src = Cursor::new(&data);
    let result = mknoddata3(&mut src).unwrap();
    assert!(matches!(result, mknoddata3::NF3REG));

    let data = [0x00, 0x00, 0x00, 0x02];
    let mut src = Cursor::new(&data);
    let result = mknoddata3(&mut src).unwrap();
    assert!(matches!(result, mknoddata3::NF3DIR));

    let data = [
        0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
    ];
    let mut src = Cursor::new(&data);
    let result = mknoddata3(&mut src).unwrap();
    assert!(matches!(result, mknoddata3::NF3BLK(_)));

    let data = [0x00, 0x00, 0x00, 0x05];
    let mut src = Cursor::new(&data);
    let result = mknoddata3(&mut src).unwrap();
    assert!(matches!(result, mknoddata3::NF3LNK));

    let data = [0x00, 0x00, 0x00, 0x08];
    let mut src = Cursor::new(&data);
    assert!(matches!(mknoddata3(&mut src), Err(Error::EnumDiscMismatch)));
}

#[test]
fn test_c_enum() {
    let data = [0x00, 0x00, 0x00, 0x06];
    let mut src = Cursor::new(&data);
    let result = variant(&mut src).unwrap();
    assert!(matches!(result, ftype3::NF3SOCK));

    let data = [0x00, 0x00, 0x00, 0x08];
    let mut src = Cursor::new(&data);
    let result = variant::<ftype3>(&mut src);
    assert!(matches!(result, Err(Error::EnumDiscMismatch)));
}
