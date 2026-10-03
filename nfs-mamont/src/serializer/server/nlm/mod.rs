//! NLM v4 XDR result serializers.
//!
//! Serializes NLM procedure responses (Lock, Unlock, Test, Cancel)
//! into XDR wire format for transmission back to the client.

use std::io;
use std::io::Write;

use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::test::Nlm4TestArgs;
use crate::nlm::{Nlm4Stats, OpaqueHandle};
use crate::serializer::files::file_handle;
use crate::serializer::{bool, i32, string, u64, variant, vector};

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

/// Serializes an [`OpaqueHandle`].
fn opaque_handle(dest: &mut impl Write, opaque: OpaqueHandle) -> io::Result<()> {
    vector(dest, opaque.as_bytes())
}

/// Serializes an [`Nlm4Lock`] as the XDR reply body.
fn lock(dest: &mut impl Write, lock: Nlm4Lock) -> io::Result<()> {
    string(dest, &lock.caller_name)?;
    file_handle(dest, lock.file_handle)?;
    opaque_handle(dest, lock.opaque_handle)?;
    i32(dest, lock.system_identifier)?;
    u64(dest, lock.lock_offset)?;
    u64(dest, lock.lock_length)
}

/// Serializes an [`Nlm4TestArgs`] as the XDR reply body.
pub fn test_args(dest: &mut impl Write, args: Nlm4TestArgs) -> io::Result<()> {
    cookie(dest, args.cookie)?;
    bool(dest, args.exclusive)?;
    lock(dest, args.lock)
}
