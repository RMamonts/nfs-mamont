use std::io::Cursor;

use super::xdr;
use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::parser::nlm::test::test;

#[test]
fn test_test() {
    let mut data = Vec::new();
    data.extend(xdr::u64_val(1));
    data.extend(xdr::bool_val(true));
    data.extend(xdr::string("host"));
    data.extend(xdr::handle(&[0x01; NFS3_FHSIZE]));
    data.extend(xdr::opaque(&[0xCD; 4]));
    data.extend(xdr::i32_val(42));
    data.extend(xdr::u64_val(100));
    data.extend(xdr::u64_val(200));

    let result = test(&mut Cursor::new(data)).unwrap();

    assert_eq!(result.cookie.raw(), 1);
    assert!(result.exclusive);
    assert_eq!(result.lock.caller_name, "host");
}

#[test]
fn test_test_insufficient_data() {
    assert!(test(&mut Cursor::new(xdr::string("h"))).is_err());
}
