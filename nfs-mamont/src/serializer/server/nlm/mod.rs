//! NLM v4 XDR result serializers.
//!
//! Serializes NLM procedure responses (Lock, Unlock, Test, Cancel)
//! into XDR wire format for transmission back to the client.

use std::io;
use std::io::Write;

use crate::consts::nlm::OPAQUE_HANDLE_SIZE;
use crate::nlm::procedures::{
    cancel::Nlm4CancelRes, lock::Nlm4LockRes, test::Nlm4TestRes, unlock::Nlm4UnlockRes,
};
use crate::nlm::{Nlm4Stats, OpaqueHandle};
use crate::serializer::{u32, u64, variant, vector};

/// Writes an NLM cookie as an XDR `hyper`.
pub fn cookie(dest: &mut impl Write, cookie: crate::nlm::cookie::Cookie) -> io::Result<()> {
    u64(dest, cookie.raw())
}

/// Writes an [`Nlm4Stats`] value as an XDR enum discriminant.
fn stat(dest: &mut impl Write, stat: Nlm4Stats) -> io::Result<()> {
    variant::<Nlm4Stats>(dest, stat)
}

/// Serializes an [`Nlm4LockRes`] as the XDR reply body for `NLMPROC4_LOCK`.
pub fn lock_res(dest: &mut impl Write, res: Nlm4LockRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.stat)
}

/// Serializes an [`Nlm4UnlockRes`] as the XDR reply body for `NLMPROC4_UNLOCK`.
pub fn unlock_res(dest: &mut impl Write, res: Nlm4UnlockRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.stat)
}

/// Serializes an [`Nlm4CancelRes`] as the XDR reply body for `NLMPROC4_CANCEL`.
pub fn cancel_res(dest: &mut impl Write, res: Nlm4CancelRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.stat)
}

pub fn opaque_handle(dest: &mut impl Write, opaque_handle: OpaqueHandle) -> io::Result<()> {
    u32(dest, OPAQUE_HANDLE_SIZE as u32)?;
    vector(dest, opaque_handle.as_bytes())
}

/// Serializes an [`Nlm4TestRes`] as the XDR reply body for `NLMPROC4_TEST`.
///
/// The reply is an XDR union: when the status is `Denied` the current
/// lock holder information is included; otherwise only the status is
/// written.
pub fn test_res(dest: &mut impl Write, res: Nlm4TestRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.test_stat.stat)?;
    if res.test_stat.stat == Nlm4Stats::Denied {
        let holder = match res.test_stat.holder {
            Some(holder) => holder,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Stat is Denied but holder is None",
                ))
            }
        };
        u32(dest, holder.exclusive as u32)?;
        u32(dest, holder.system_identifier as u32)?;
        vector(dest, holder.opaque_handle.as_bytes())?;
        u64(dest, holder.lock_offset)?;
        u64(dest, holder.lock_length)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
    use crate::serializer::server::nlm::{cancel_res, lock_res, test_res, unlock_res};

    fn cookie(val: u64) -> Cookie {
        Cookie::new(val)
    }

    #[test]
    fn lock_res_serializes_cookie_and_granted() {
        let mut buf = Cursor::new(vec![0u8; 12]);
        let res = Nlm4LockRes { cookie: cookie(0x0102030405060708), stat: Nlm4Stats::Granted };
        lock_res(&mut buf, res).unwrap();
        assert_eq!(
            buf.into_inner(),
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // cookie
                0x00, 0x00, 0x00, 0x00, // Granted = 0
            ]
        );
    }

    #[test]
    fn lock_res_serializes_cookie_and_denied() {
        let mut buf = Cursor::new(vec![0u8; 12]);
        let res = Nlm4LockRes { cookie: cookie(0), stat: Nlm4Stats::Denied };
        lock_res(&mut buf, res).unwrap();
        assert_eq!(
            buf.into_inner(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // cookie = 0
                0x00, 0x00, 0x00, 0x01, // Denied = 1
            ]
        );
    }

    #[test]
    fn unlock_res_serializes_cookie_and_stat() {
        let mut buf = Cursor::new(vec![0u8; 12]);
        let res = Nlm4UnlockRes { cookie: cookie(7), stat: Nlm4Stats::Granted };
        unlock_res(&mut buf, res).unwrap();
        assert_eq!(
            buf.into_inner(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, // cookie = 7
                0x00, 0x00, 0x00, 0x00, // Granted = 0
            ]
        );
    }

    #[test]
    fn cancel_res_serializes_cookie_and_stat() {
        let mut buf = Cursor::new(vec![0u8; 12]);
        let res = Nlm4CancelRes { cookie: cookie(u64::MAX), stat: Nlm4Stats::Denied };
        cancel_res(&mut buf, res).unwrap();
        assert_eq!(
            buf.into_inner(),
            [
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // cookie = u64::MAX
                0x00, 0x00, 0x00, 0x01, // Denied = 1
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
        assert_eq!(
            buf.into_inner(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64, // cookie = 100
                0x00, 0x00, 0x00, 0x00, // Granted = 0
            ]
        );
    }

    #[test]
    fn test_res_granted_ignores_holder() {
        let mut buf = Cursor::new(vec![0u8; 12]);
        let res = Nlm4TestRes {
            cookie: cookie(100),
            test_stat: Nlm4TestReply {
                stat: Nlm4Stats::Granted,
                holder: Some(Nlm4Holder::new(
                    true,
                    1,
                    OpaqueHandle::new([0; OPAQUE_HANDLE_SIZE].to_vec()).unwrap(),
                    0,
                    0,
                )),
            },
        };
        test_res(&mut buf, res).unwrap();
        assert_eq!(
            buf.into_inner(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64, // cookie = 100
                0x00, 0x00, 0x00, 0x00, // Granted = 0
            ]
        );
    }

    #[test]
    fn test_res_denied_serializes_full_holder() {
        let holder = Nlm4Holder::new(
            true,                                                            // exclusive
            12345,                                                           // system_identifier
            OpaqueHandle::new([0xAB; OPAQUE_HANDLE_SIZE].to_vec()).unwrap(), // opaque_handle
            99,                                                              // lock_offset
            200,                                                             // lock_length
        );
        let res = Nlm4TestRes {
            cookie: cookie(0xDEADBEEF),
            test_stat: Nlm4TestReply { stat: Nlm4Stats::Denied, holder: Some(holder) },
        };

        let mut buf = Cursor::new(vec![0u8; 1064]);
        test_res(&mut buf, res).unwrap();

        let bytes = buf.into_inner();
        assert_eq!(&bytes[0..8], [0x00, 0x00, 0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]); // cookie
        assert_eq!(&bytes[8..12], [0x00, 0x00, 0x00, 0x01]); // Denied = 1
        assert_eq!(&bytes[12..16], [0x00, 0x00, 0x00, 0x01]); // exclusive = true
        assert_eq!(&bytes[16..20], [0x00, 0x00, 0x30, 0x39]); // system_identifier = 12345
        assert_eq!(&bytes[20..24], [0x00, 0x00, 0x04, 0x00]); // opaque_handle length = 1024
        assert_eq!(&bytes[24..24 + OPAQUE_HANDLE_SIZE], [0xAB; OPAQUE_HANDLE_SIZE]); // opaque_handle bytes
                                                                                     // OPAQUE_HANDLE_SIZE % 4 == 0, so no trailing padding
        let offset_off = 24 + OPAQUE_HANDLE_SIZE;
        assert_eq!(
            &bytes[offset_off..offset_off + 8],
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x63]
        ); // lock_offset = 99
        let len_off = offset_off + 8;
        assert_eq!(&bytes[len_off..len_off + 8], [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC8]);
        // lock_length = 200
    }

    #[test]
    fn test_res_denied_holder_has_correct_offset_in_buffer() {
        let holder = Nlm4Holder::new(
            false,
            999,
            OpaqueHandle::new([0x01; OPAQUE_HANDLE_SIZE].to_vec()).unwrap(),
            0,
            0,
        );
        let res = Nlm4TestRes {
            cookie: cookie(0),
            test_stat: Nlm4TestReply { stat: Nlm4Stats::Denied, holder: Some(holder) },
        };

        let mut buf = Cursor::new(vec![0u8; 1064]);
        test_res(&mut buf, res).unwrap();

        let bytes = buf.into_inner();
        assert_eq!(&bytes[12..16], [0x00, 0x00, 0x00, 0x00]); // exclusive = false
        assert_eq!(&bytes[16..20], [0x00, 0x00, 0x03, 0xE7]); // system_identifier = 999
    }
}
