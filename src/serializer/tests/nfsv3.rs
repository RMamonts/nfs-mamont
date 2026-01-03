use crate::nfsv3::{createhow3, devicedata3};
use crate::nfsv3::{ftype3, mknoddata3};
use crate::nfsv3::{nfs_fh3, nfstime3, sattr3, set_atime, set_mtime, specdata3};
use crate::serializer::nfs::{
    createhow3, devicedata3, diropargs3, mknoddata3, nfs_fh3, nfstime, sattr3, set_atime,
    set_mtime, specdata3, symlinkdata3,
};
use crate::serializer::variant;
use std::io::Cursor;

#[test]
fn test_specdata3() {
    let mut init = Cursor::new([1u8; 9]);
    let arg = specdata3 { specdata1: 0, specdata2: 255 };
    specdata3(&mut init, arg).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 0, 0, 0, 0, 255, 1]);
}

#[test]
fn test_nfstime() {
    let mut init = Cursor::new([1u8; 9]);
    let arg = nfstime3 { seconds: u32::MAX, nseconds: u32::MAX };
    nfstime(&mut init, arg).unwrap();
    assert_eq!(init.into_inner(), [255, 255, 255, 255, 255, 255, 255, 255, 1]);
}

#[test]
fn test_atime() {
    let mut init = Cursor::new([1u8; 21]);
    set_atime(&mut init, set_atime::DONT_CHANGE).unwrap();
    set_atime(&mut init, set_atime::SET_TO_SERVER_TIME).unwrap();
    set_atime(&mut init, set_atime::SET_TO_CLIENT_TIME(nfstime3 { seconds: 255, nseconds: 0 }))
        .unwrap();
    assert_eq!(
        init.into_inner(),
        [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 255, 0, 0, 0, 0, 1]
    );
}

#[test]
fn test_mtime() {
    let mut init = Cursor::new([1u8; 21]);
    set_mtime(&mut init, set_mtime::DONT_CHANGE).unwrap();
    set_mtime(&mut init, set_mtime::SET_TO_SERVER_TIME).unwrap();
    set_mtime(&mut init, set_mtime::SET_TO_CLIENT_TIME(nfstime3 { seconds: 255, nseconds: 0 }))
        .unwrap();
    assert_eq!(
        init.into_inner(),
        [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 255, 0, 0, 0, 0, 1]
    );
}

#[test]
fn test_sattr3() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02,
    ];
    let mut init = Cursor::new([2u8; 37]);
    sattr3(
        &mut init,
        sattr3 {
            mode: None,
            uid: Some(1),
            gid: None,
            size: Some(1),
            atime: set_atime::DONT_CHANGE,
            mtime: set_mtime::SET_TO_SERVER_TIME,
        },
    )
    .unwrap();
    assert_eq!(init.into_inner(), data)
}

#[test]
fn test_nfs_fh3() {
    let mut init = Cursor::new([1u8; 13]);
    let fh = nfs_fh3 { data: [1, 4, 0, 8, 5, 6, 9, 255] };
    nfs_fh3(&mut init, fh).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 8, 1, 4, 0, 8, 5, 6, 9, 255, 1]);
}

#[test]
fn test_diroparg() {
    let data = [
        0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x03, b'a', b'b', b'c', 0x00, 0x01,
    ];
    let mut init = Cursor::new([1u8; 21]);
    diropargs3(&mut init, nfs_fh3 { data: [1, 2, 0, 0, 0, 0, 0, 0] }, "abc".to_string()).unwrap();
    assert_eq!(init.into_inner(), data);
}

#[test]
fn test_createhow3() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x01, 0x20, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01,
    ];
    let mut init = Cursor::new([1u8; 93]);
    createhow3(
        &mut init,
        createhow3::UNCHECKED(sattr3 {
            mode: None,
            uid: Some(1),
            gid: None,
            size: Some(1),
            atime: set_atime::DONT_CHANGE,
            mtime: set_mtime::SET_TO_SERVER_TIME,
        }),
    )
    .unwrap();
    createhow3(
        &mut init,
        createhow3::GUARDED(sattr3 {
            mode: None,
            uid: Some(1),
            gid: None,
            size: Some(1),
            atime: set_atime::DONT_CHANGE,
            mtime: set_mtime::SET_TO_SERVER_TIME,
        }),
    )
    .unwrap();
    createhow3(&mut init, createhow3::EXCLUSIVE([1, 32, 0, 0, 0, 0, 0, 0])).unwrap();
    assert_eq!(init.into_inner(), data)
}

#[test]
fn test_symlink() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, b'a', b'/', b'b', 0x00, 0x01,
    ];
    let mut init = Cursor::new([1u8; 45]);
    symlinkdata3(
        &mut init,
        sattr3 {
            mode: None,
            uid: Some(1),
            gid: None,
            size: Some(1),
            atime: set_atime::DONT_CHANGE,
            mtime: set_mtime::SET_TO_SERVER_TIME,
        },
        "a/b".to_string(),
    )
    .unwrap();
    assert_eq!(init.into_inner(), data);
}

#[test]
fn test_devdata() {
    let data = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0, 0, 0, 255, 0, 0, 0, 0, 1,
    ];
    let mut init = Cursor::new([1u8; 45]);
    let dev = devicedata3 {
        dev_attributes: sattr3 {
            mode: None,
            uid: Some(1),
            gid: None,
            size: Some(1),
            atime: set_atime::DONT_CHANGE,
            mtime: set_mtime::SET_TO_SERVER_TIME,
        },
        spec: specdata3 { specdata1: 255, specdata2: 0 },
    };
    devicedata3(&mut init, dev).unwrap();
    assert_eq!(init.into_inner(), data);
}

#[test]
fn test_mknoddata3() {
    let data = [
        0, 0, 0, 1, 0, 0, 0, 3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0,
        0, 7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 1,
    ];
    let mut init = Cursor::new([1u8; 93]);

    mknoddata3(&mut init, mknoddata3::NF3REG).unwrap();
    mknoddata3(
        &mut init,
        mknoddata3::NF3BLK(devicedata3 {
            dev_attributes: sattr3 {
                mode: None,
                uid: Some(1),
                gid: None,
                size: Some(1),
                atime: set_atime::DONT_CHANGE,
                mtime: set_mtime::SET_TO_SERVER_TIME,
            },
            spec: specdata3 { specdata1: 255, specdata2: 0 },
        }),
    )
    .unwrap();
    mknoddata3(
        &mut init,
        mknoddata3::NF3FIFO(sattr3 {
            mode: None,
            uid: Some(1),
            gid: None,
            size: Some(1),
            atime: set_atime::DONT_CHANGE,
            mtime: set_mtime::SET_TO_SERVER_TIME,
        }),
    )
    .unwrap();
    assert_eq!(init.into_inner(), data);
}

#[test]
fn test_ftype() {
    let mut init = Cursor::new([1u8; 9]);
    variant(&mut init, ftype3::NF3SOCK).unwrap();
    variant(&mut init, ftype3::NF3REG).unwrap();
    assert_eq!(init.into_inner(), [0, 0, 0, 6, 0, 0, 0, 1, 1]);
}
