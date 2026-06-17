//! Parsing of NLMv4 procedure arguments from incoming RPC calls.

use crate::consts::nlm;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::OpaqueHandle;
use crate::parser::nfsv3::file;
use crate::parser::primitive::{i32, string_max_size, u64, vector};
use crate::parser::{Error, Result};
use std::io::Read;

pub mod cancel;
pub mod lock;
pub mod test;
pub mod unlock;

/// Decodes the lock-owner identifier from an NLM request.
/// The wire encoding is a variable-length opaque capped at [`OPAQUE_HANDLE_SIZE`](nlm::OPAQUE_HANDLE_SIZE).
pub fn opaque_handle(src: &mut impl Read) -> Result<OpaqueHandle> {
    OpaqueHandle::new(vector(src)?).map_err(|_| Error::BadFileHandle)
}

/// Decodes the lock-arguments block shared by every LOCK/UNLOCK/TEST/CANCEL request.
pub fn parse_lock(src: &mut impl Read) -> Result<Nlm4Lock> {
    Nlm4Lock::new(
        string_max_size(src, nlm::LM_MAXSTRLEN)?,
        file::handle(src)?,
        opaque_handle(src)?,
        i32(src)?,
        u64(src)?,
        u64(src)?,
    )
    .map_err(|_| Error::BadFileHandle)
}

/// Test helpers that encode NLM procedure arguments in XDR format.
///
/// Wraps the production [`serializer`](crate::serializer) to build test data.
#[cfg(test)]
pub(crate) mod xdr {
    use std::io::Write;

    use crate::consts::nfsv3::NFS3_FHSIZE;
    use crate::serializer;
    use byteorder::{BigEndian, WriteBytesExt};

    pub fn string(s: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        serializer::string(&mut buf, s).unwrap();
        buf
    }

    pub fn handle(bytes: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        serializer::u32(&mut buf, NFS3_FHSIZE as u32).unwrap();
        buf.write_all(bytes).unwrap();
        buf
    }

    pub fn opaque(bytes: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        serializer::vector(&mut buf, bytes).unwrap();
        buf
    }

    pub fn i32_val(v: i32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.write_i32::<BigEndian>(v).unwrap();
        buf
    }

    pub fn u32_val(v: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        serializer::u32(&mut buf, v).unwrap();
        buf
    }

    pub fn u64_val(v: u64) -> Vec<u8> {
        let mut buf = Vec::new();
        serializer::u64(&mut buf, v).unwrap();
        buf
    }

    pub fn bool_val(v: bool) -> Vec<u8> {
        let mut buf = Vec::new();
        serializer::bool(&mut buf, v).unwrap();
        buf
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn make_lock_bytes(
        caller_name: &str,
        fh: &[u8],
        oh: &[u8],
        svid: i32,
        offset: u64,
        length: u64,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend(xdr::string(caller_name));
        data.extend(xdr::handle(fh));
        data.extend(xdr::opaque(oh));
        data.extend(xdr::i32_val(svid));
        data.extend(xdr::u64_val(offset));
        data.extend(xdr::u64_val(length));
        data
    }

    #[test]
    fn parse_lock_success() {
        let data = make_lock_bytes("host", &[0xAB; 8], &[0xCD; 4], 12345, 0, 100);
        let lock = parse_lock(&mut Cursor::new(data)).unwrap();

        assert_eq!(lock.caller_name, "host");
        assert_eq!(lock.system_identifier, 12345);
        assert_eq!(lock.lock_length, 100);
    }

    #[test]
    fn parse_lock_empty_caller_name() {
        let data = make_lock_bytes("", &[0; 8], &[0; 4], 0, 0, 0);
        assert!(parse_lock(&mut Cursor::new(data)).is_err());
    }

    #[test]
    fn opaque_handle_normal() {
        let data = xdr::opaque(&[0xAB, 0xCD, 0xEF]);
        let oh = opaque_handle(&mut Cursor::new(data)).unwrap();
        assert_eq!(oh.as_bytes()[..3], [0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn opaque_handle_zero_length() {
        let data = xdr::opaque(&[]);
        let oh = opaque_handle(&mut Cursor::new(data)).unwrap();
        assert!(oh.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn opaque_handle_max_size() {
        let data = xdr::opaque(&[0x42; 1024]);
        let oh = opaque_handle(&mut Cursor::new(data)).unwrap();
        assert_eq!(oh.as_bytes()[..1024], [0x42; 1024]);
    }

    #[test]
    fn opaque_handle_too_large() {
        let data = xdr::opaque(&[0; 1025]);
        assert!(matches!(opaque_handle(&mut Cursor::new(data)), Err(Error::BadFileHandle)));
    }

    #[test]
    fn opaque_handle_insufficient_data() {
        let data = xdr::u32_val(10);
        assert!(matches!(opaque_handle(&mut Cursor::new(data)), Err(Error::IO(_))));
    }

    #[test]
    fn parse_lock_bad_file_handle() {
        let mut data = Vec::new();
        data.extend(xdr::string("host"));
        data.extend(xdr::i32_val(3));
        assert!(matches!(parse_lock(&mut Cursor::new(data)), Err(Error::BadFileHandle)));
    }

    #[test]
    fn parse_lock_insufficient_data() {
        let data = xdr::string("host");
        assert!(matches!(parse_lock(&mut Cursor::new(data)), Err(Error::IO(_))));
    }
}
