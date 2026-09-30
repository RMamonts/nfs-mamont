use std::io::Cursor;

use super::xdr;
use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::parser::nlm::lock::lock;

#[test]
fn test_lock() {
    let mut data = Vec::new();
    data.extend(xdr::u64_val(7));
    data.extend(xdr::bool_val(true));
    data.extend(xdr::bool_val(false));
    data.extend(xdr::string("client"));
    data.extend(xdr::handle(&[0x11; NFS3_FHSIZE]));
    data.extend(xdr::opaque(&[0xAA; 4]));
    data.extend(xdr::i32_val(99));
    data.extend(xdr::u64_val(0));
    data.extend(xdr::u64_val(4096));
    data.extend(xdr::bool_val(false));
    data.extend(xdr::u32_val(3));

    let result = lock(&mut Cursor::new(data)).unwrap();

    assert_eq!(result.cookie.raw(), 7);
    assert!(result.block);
    assert!(!result.exclusive);
    assert!(!result.reclaim);
    assert_eq!(result.state, 3);
    assert_eq!(result.lock.caller_name, "client");
}

#[test]
fn test_lock_insufficient_data() {
    assert!(lock(&mut Cursor::new(xdr::string("a"))).is_err());
}
