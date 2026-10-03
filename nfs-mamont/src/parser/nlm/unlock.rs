//! Implements parsing for [`Nlm4UnlockArgs`] structure.

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::unlock::Nlm4UnlockArgs;
use crate::parser::nlm::parse_lock;
use crate::parser::primitive::u64;
use crate::parser::Result;
use std::io::Read;

/// Parses the arguments for an NLMv4 `UNLOCK` operation from the provided `Read` source.
pub fn unlock(src: &mut impl Read) -> Result<Nlm4UnlockArgs> {
    Ok(Nlm4UnlockArgs { cookie: Cookie::new(u64(src)?), lock: parse_lock(src)? })
}
