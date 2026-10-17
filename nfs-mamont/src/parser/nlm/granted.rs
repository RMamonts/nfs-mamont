// Implements parsing for Nlm4GrantedRes struture.

use std::io::Read;

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::granted::Nlm4GrantedRes;
use crate::nlm::Nlm4Stats;
use crate::parser::primitive::{u64, variant};

/// Parses the arguments for an NLMv4 `CANCEL` operation from the provided `Read` source.
pub fn granted(src: &mut impl Read) -> crate::parser::Result<Nlm4GrantedRes> {
    Ok(Nlm4GrantedRes { cookie: Cookie::new(u64(src)?), stat: variant::<Nlm4Stats>(src)? })
}
