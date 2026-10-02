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
