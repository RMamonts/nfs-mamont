use std::io::Cursor;

use super::xdr;
use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::nlm::Name;
use crate::parser::nlm::unlock::unlock;

#[test]
fn test_unlock() {
    let mut data = Vec::new();
    data.extend(xdr::u64_val(42));
    data.extend(xdr::string("nfs-client"));
    data.extend(xdr::handle(&[0xFE; NFS3_FHSIZE]));
    data.extend(xdr::opaque(&[0xAB, 0xCD]));
    data.extend(xdr::i32_val(-1));
    data.extend(xdr::u64_val(50));
    data.extend(xdr::u64_val(0));

    let result = unlock(&mut Cursor::new(data)).unwrap();
    let name = Name::new("nfs-client".to_string()).unwrap();

    assert_eq!(result.cookie.raw(), 42);
    assert_eq!(result.lock.caller_name, name);
    assert_eq!(result.lock.system_identifier, -1);
}

#[test]
fn test_unlock_insufficient_data() {
    assert!(unlock(&mut Cursor::new(xdr::string("test"))).is_err());
}
