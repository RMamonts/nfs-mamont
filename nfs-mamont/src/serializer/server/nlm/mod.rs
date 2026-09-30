//! NLM v4 XDR result serializers.
//!
//! Serializes NLM procedure responses (Lock, Unlock, Test, Cancel)
//! into XDR wire format for transmission back to the client.

use std::io;
use std::io::Write;

use crate::nlm::Nlm4Stats;
use crate::serializer::{u64, variant};

mod cancel;
mod lock;
mod test;
mod unlock;
#[cfg(test)]
mod tests;

pub use cancel::cancel_res;
pub use lock::lock_res;
pub use test::test_res;
pub use unlock::unlock_res;

/// Writes an NLM cookie as an XDR `hyper`.
fn cookie(dest: &mut impl Write, cookie: crate::nlm::cookie::Cookie) -> io::Result<()> {
    u64(dest, cookie.raw())
}

/// Writes an [`Nlm4Stats`] value as an XDR enum discriminant.
fn stat(dest: &mut impl Write, stat: Nlm4Stats) -> io::Result<()> {
    variant::<Nlm4Stats>(dest, stat)
}