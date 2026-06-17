//! Implements parsing for [`Nlm4UnlockArgs`] structure.

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::unlock::Nlm4UnlockArgs;
use crate::parser::nlm::parse_lock;
use crate::parser::primitive::u64;
use crate::parser::Result;
use std::io::Read;

/// Parses the arguments for an NLMv4 `UNLOCK` operation from the provided `Read` source.
pub fn unlock(src: &mut impl Read) -> Result<Nlm4UnlockArgs> {
    let cookie = Cookie::new(u64(src)?);
    let lock = parse_lock(src)?;
    Ok(Nlm4UnlockArgs { cookie, lock })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::nlm::xdr;

    #[test]
    fn test_unlock() {
        let mut data = Vec::new();
        data.extend(xdr::string("nfs-client"));
        data.extend(xdr::handle(&[0xFE; 8]));
        data.extend(xdr::opaque(&[0xAB, 0xCD]));
        data.extend(xdr::i32_val(-1));
        data.extend(xdr::u64_val(50));
        data.extend(xdr::u64_val(0));
        data.extend(xdr::u64_val(42));

        let result = super::unlock(&mut Cursor::new(data)).unwrap();

        assert_eq!(result.cookie.raw(), 42);
        assert_eq!(result.lock.caller_name, "nfs-client");
        assert_eq!(result.lock.system_identifier, -1);
    }

    #[test]
    fn test_unlock_insufficient_data() {
        assert!(super::unlock(&mut Cursor::new(xdr::string("test"))).is_err());
    }
}
