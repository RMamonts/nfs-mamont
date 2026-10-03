mod cancel;
mod lock;
mod test;
mod unlock;
mod xdr;
pub mod granted;

use std::io::Cursor;

use super::{opaque_handle, parse_lock};
use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::parser::Error;

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
    let data = make_lock_bytes("host", &[0xAB; NFS3_FHSIZE], &[0xCD; 4], 12345, 0, 100);
    let lock = parse_lock(&mut Cursor::new(data)).unwrap();

    assert_eq!(lock.caller_name, "host");
    assert_eq!(lock.system_identifier, 12345);
    assert_eq!(lock.lock_length, 100);
}

#[test]
fn parse_lock_empty_caller_name() {
    let data = make_lock_bytes("", &[0; NFS3_FHSIZE], &[0; 4], 0, 0, 0);
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
