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
