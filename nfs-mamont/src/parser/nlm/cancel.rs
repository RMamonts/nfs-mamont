//! Implements parsing for [`Nlm4CancelArgs`] structure.

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::cancel::Nlm4CancelArgs;
use crate::parser::nlm::parse_lock;
use crate::parser::primitive::{bool, u64};
use crate::parser::Result;
use std::io::Read;

/// Parses the arguments for an NLMv4 `CANCEL` operation from the provided `Read` source.
pub fn cancel(src: &mut impl Read) -> Result<Nlm4CancelArgs> {
    Ok(Nlm4CancelArgs {
        cookie: Cookie::new(u64(src)?),
        block: bool(src)?,
        exclusive: bool(src)?,
        lock: parse_lock(src)?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::nlm::xdr;

    #[test]
    fn test_cancel() {
        let mut data = Vec::new();
        data.extend(xdr::string("hostname"));
        data.extend(xdr::handle(&[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]));
        data.extend(xdr::opaque(&[0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA, 0x99, 0x88]));
        data.extend(xdr::i32_val(777));
        data.extend(xdr::u64_val(1024));
        data.extend(xdr::u64_val(512));
        data.extend(xdr::u64_val(0));
        data.extend(xdr::bool_val(false));
        data.extend(xdr::bool_val(true));

        let result = super::cancel(&mut Cursor::new(data)).unwrap();

        assert!(result.cookie.is_zero());
        assert!(!result.block);
        assert!(result.exclusive);
        assert_eq!(result.lock.caller_name, "hostname");
    }

    #[test]
    fn test_cancel_insufficient_data() {
        assert!(super::cancel(&mut Cursor::new(vec![0u8; 1])).is_err());
    }
}
