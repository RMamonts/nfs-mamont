//! Implements parsing for [`Nlm4TestArgs`] structure.

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::test::Nlm4TestArgs;
use crate::parser::nlm::parse_lock;
use crate::parser::primitive::{bool, u64};
use crate::parser::Result;
use std::io::Read;

/// Parses the arguments for an NLMv4 `TEST` operation from the provided `Read` source.
pub fn test(src: &mut impl Read) -> Result<Nlm4TestArgs> {
    Ok(Nlm4TestArgs {
        cookie: Cookie::new(u64(src)?),
        exclusive: bool(src)?,
        lock: parse_lock(src)?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::nlm::xdr;

    #[test]
    fn test_test() {
        let mut data = Vec::new();
        data.extend(xdr::string("host"));
        data.extend(xdr::handle(&[0x01; 8]));
        data.extend(xdr::opaque(&[0xCD; 4]));
        data.extend(xdr::i32_val(42));
        data.extend(xdr::u64_val(100));
        data.extend(xdr::u64_val(200));
        data.extend(xdr::u64_val(1));
        data.extend(xdr::bool_val(true));

        let result = super::test(&mut Cursor::new(data)).unwrap();

        assert_eq!(result.cookie.raw(), 1);
        assert!(result.exclusive);
        assert_eq!(result.lock.caller_name, "host");
    }

    #[test]
    fn test_test_insufficient_data() {
        assert!(super::test(&mut Cursor::new(xdr::string("h"))).is_err());
    }
}
