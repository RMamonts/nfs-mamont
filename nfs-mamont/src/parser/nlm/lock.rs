//! Implements parsing for [`Nlm4LockArgs`] structure.
use std::io::Read;

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::lock::Nlm4LockArgs;
use crate::parser::nlm::parse_lock;
use crate::parser::primitive::{bool, u32, u64};
use crate::parser::Result;

/// Parses the arguments for an NLMv4 `LOCK` operation from the provided `Read` source.
pub fn lock(src: &mut impl Read) -> Result<Nlm4LockArgs> {
    Ok(Nlm4LockArgs {
        cookie: Cookie::new(u64(src)?),
        block: bool(src)?,
        exclusive: bool(src)?,
        lock: parse_lock(src)?,
        reclaim: bool(src)?,
        state: u32(src)?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::nlm::xdr;

    #[test]
    fn test_lock() {
        let mut data = Vec::new();
        data.extend(xdr::u64_val(7));
        data.extend(xdr::bool_val(true));
        data.extend(xdr::bool_val(false));
        data.extend(xdr::string("client"));
        data.extend(xdr::handle(&[0x11; 8]));
        data.extend(xdr::opaque(&[0xAA; 4]));
        data.extend(xdr::i32_val(99));
        data.extend(xdr::u64_val(0));
        data.extend(xdr::u64_val(4096));
        data.extend(xdr::bool_val(false));
        data.extend(xdr::u32_val(3));

        let result = super::lock(&mut Cursor::new(data)).unwrap();

        assert_eq!(result.cookie.raw(), 7);
        assert!(result.block);
        assert!(!result.exclusive);
        assert!(!result.reclaim);
        assert_eq!(result.state, 3);
        assert_eq!(result.lock.caller_name, "client");
    }

    #[test]
    fn test_lock_insufficient_data() {
        assert!(super::lock(&mut Cursor::new(xdr::string("a"))).is_err());
    }
}
