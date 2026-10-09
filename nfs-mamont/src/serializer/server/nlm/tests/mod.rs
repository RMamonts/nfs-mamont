use std::io::Cursor;

use crate::consts::nlm::OPAQUE_HANDLE_SIZE;
use crate::nlm::cookie::Cookie;
use crate::nlm::holder::Nlm4Holder;
use crate::nlm::procedures::{
    cancel::Nlm4CancelRes,
    lock::Nlm4LockRes,
    test::{Nlm4TestReply, Nlm4TestRes},
    unlock::Nlm4UnlockRes,
};
use crate::nlm::{Nlm4Stats, OpaqueHandle};

use super::{cancel_res, lock_res, test_res, unlock_res};

fn cookie(val: u64) -> Cookie {
    Cookie::new(val)
}

#[test]
fn lock_res_serializes_cookie_and_granted() {
    let mut buf = Cursor::new(vec![0u8; 12]);
    let res = Nlm4LockRes { cookie: cookie(0x0102030405060708), stat: Nlm4Stats::Granted };
    lock_res(&mut buf, res).unwrap();
    #[rustfmt::skip]
    assert_eq!(
        buf.into_inner(),
        [
            // cookie
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            // Granted = 0
            0x00, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn lock_res_serializes_cookie_and_denied() {
    let mut buf = Cursor::new(vec![0u8; 12]);
    let res = Nlm4LockRes { cookie: cookie(0), stat: Nlm4Stats::Denied };
    lock_res(&mut buf, res).unwrap();
    #[rustfmt::skip]
    assert_eq!(
        buf.into_inner(),
        [
            // cookie = 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Denied = 1
            0x00, 0x00, 0x00, 0x01,
        ]
    );
}

#[test]
fn unlock_res_serializes_cookie_and_stat() {
    let mut buf = Cursor::new(vec![0u8; 12]);
    let res = Nlm4UnlockRes { cookie: cookie(7), stat: Nlm4Stats::Granted };
    unlock_res(&mut buf, res).unwrap();
    #[rustfmt::skip]
    assert_eq!(
        buf.into_inner(),
        [
            // cookie = 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07,
            // Granted = 0
            0x00, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn cancel_res_serializes_cookie_and_stat() {
    let mut buf = Cursor::new(vec![0u8; 12]);
    let res = Nlm4CancelRes { cookie: cookie(u64::MAX), stat: Nlm4Stats::Denied };
    cancel_res(&mut buf, res).unwrap();
    #[rustfmt::skip]
    assert_eq!(
        buf.into_inner(),
        [
            // cookie = u64::MAX
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            // Denied = 1
            0x00, 0x00, 0x00, 0x01,
        ]
    );
}

#[test]
fn test_res_granted_serializes_cookie_and_granted_no_holder() {
    let mut buf = Cursor::new(vec![0u8; 12]);
    let res = Nlm4TestRes {
        cookie: cookie(100),
        test_stat: Nlm4TestReply { stat: Nlm4Stats::Granted, holder: None },
    };
    test_res(&mut buf, res).unwrap();
    #[rustfmt::skip]
    assert_eq!(
        buf.into_inner(),
        [
            // cookie = 100
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64,
            // Granted = 0
            0x00, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn test_res_granted_ignores_holder() {
    let mut buf = Cursor::new(vec![0u8; 12]);
    let res = Nlm4TestRes {
        cookie: cookie(100),
        test_stat: Nlm4TestReply { stat: Nlm4Stats::Granted, holder: None },
    };
    test_res(&mut buf, res).unwrap();
    #[rustfmt::skip]
    assert_eq!(
        buf.into_inner(),
        [
            // cookie = 100
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64,
            // Granted = 0
            0x00, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn test_res_denied_serializes_full_holder() {
    let holder = Nlm4Holder {
        exclusive: true,
        system_identifier: 12345,
        opaque_handle: OpaqueHandle::new([0xAB; OPAQUE_HANDLE_SIZE].to_vec()).unwrap(),
        lock_offset: 99,
        lock_length: 200,
    };
    let res = Nlm4TestRes {
        cookie: cookie(0xDEADBEEF),
        test_stat: Nlm4TestReply { stat: Nlm4Stats::Denied, holder: Some(holder) },
    };

    let mut buf = Cursor::new(vec![0u8; 1064]);
    test_res(&mut buf, res).unwrap();

    let bytes = buf.into_inner();
    // cookie
    assert_eq!(&bytes[0..8], [0x00, 0x00, 0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]);
    // Denied = 1
    assert_eq!(&bytes[8..12], [0x00, 0x00, 0x00, 0x01]);
    // exclusive = true
    assert_eq!(&bytes[12..16], [0x00, 0x00, 0x00, 0x01]);
    // system_identifier = 12345
    assert_eq!(&bytes[16..20], [0x00, 0x00, 0x30, 0x39]);
    // opaque_handle length = 1024
    assert_eq!(&bytes[20..24], [0x00, 0x00, 0x04, 0x00]);
    // opaque_handle bytes
    assert_eq!(&bytes[24..24 + OPAQUE_HANDLE_SIZE], [0xAB; OPAQUE_HANDLE_SIZE]);
    let offset_off = 24 + OPAQUE_HANDLE_SIZE;
    assert_eq!(
        &bytes[offset_off..offset_off + 8],
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x63]
    );
    let len_off = offset_off + 8;
    assert_eq!(&bytes[len_off..len_off + 8], [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC8]);
}

#[test]
fn test_res_denied_holder_has_correct_offset_in_buffer() {
    let holder = Nlm4Holder {
        exclusive: false,
        system_identifier: 999,
        opaque_handle: OpaqueHandle::new([0x01; OPAQUE_HANDLE_SIZE].to_vec()).unwrap(),
        lock_offset: 0,
        lock_length: 0,
    };
    let res = Nlm4TestRes {
        cookie: cookie(0),
        test_stat: Nlm4TestReply { stat: Nlm4Stats::Denied, holder: Some(holder) },
    };

    let mut buf = Cursor::new(vec![0u8; 1064]);
    test_res(&mut buf, res).unwrap();

    let bytes = buf.into_inner();
    // exclusive = false
    assert_eq!(&bytes[12..16], [0x00, 0x00, 0x00, 0x00]);
    // system_identifier = 999
    assert_eq!(&bytes[16..20], [0x00, 0x00, 0x03, 0xE7]);
}
