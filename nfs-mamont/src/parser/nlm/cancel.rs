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
