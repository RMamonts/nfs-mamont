//! NLM v4 XDR result serializers.
//!
//! Serializes NLM procedure responses (Lock, Unlock, Test, Cancel)
//! into XDR wire format for transmission back to the client.

use std::io;
use std::io::Write;

use crate::nlm::holder::Nlm4Holder;
use crate::nlm::Nlm4Stats;
use crate::serializer::{u32, u64, variant, vector};

mod cancel;
mod lock;
mod test;
#[cfg(test)]
mod tests;
mod unlock;

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

fn holder(dest: &mut impl Write, holder: Nlm4Holder) -> io::Result<()> {
    u32(dest, holder.exclusive as u32)
        .and_then(|_| u32(dest, holder.system_identifier as u32))
        .and_then(|_| vector(dest, holder.opaque_handle.as_bytes()))
        .and_then(|_| u64(dest, holder.lock_offset))
        .and_then(|_| u64(dest, holder.lock_length))
}
