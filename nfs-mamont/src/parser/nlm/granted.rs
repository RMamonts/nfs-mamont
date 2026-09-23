// Implements parsing for Nlm4GrantedRes struture.

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::granted::Nlm4GrantedRes;
use crate::parser::nlm::nlm_stat;
use crate::parser::primitive::u64;
use std::io::Read;

/// Parses the arguments for an NLMv4 `CANCEL` operation from the provided `Read` source.
pub fn granted(src: &mut impl Read) -> crate::parser::Result<Nlm4GrantedRes> {
    Ok(Nlm4GrantedRes { cookie: Cookie::new(u64(src)?), stat: nlm_stat(src)? })
}

#[cfg(test)]
mod tests {
    use crate::nlm::Nlm4Stats;
    use crate::parser::nlm::xdr;
    use std::io::Cursor;

    #[test]
    fn test_granted() {
        let mut data = Vec::new();
        // Cookie
        data.extend(xdr::u64_val(7));
        // Nlm4Stats::Granted
        data.extend(xdr::u32_val(0));

        let result = super::granted(&mut Cursor::new(data)).unwrap();

        assert_eq!(result.cookie.raw(), 7);
        assert_eq!(result.stat, Nlm4Stats::Granted);
    }
}
