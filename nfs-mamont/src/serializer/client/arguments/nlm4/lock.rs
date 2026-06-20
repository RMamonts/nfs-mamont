use std::io;
use std::io::Write;

use crate::nlm::procedures::lock;
use crate::serializer::client::arguments::nlm4::cancel::nlm_lock;
use crate::serializer::server::nlm::cookie;
use crate::serializer::{bool, u32};

/// Serializes the arguments [`lock::Nlm4LockArgs`] for a Mount `LOCK` operation to the provided `Write` destination.
pub fn lock_args(dest: &mut impl Write, arg: lock::Nlm4LockArgs) -> io::Result<()> {
    cookie(dest, arg.cookie)?;
    bool(dest, arg.block)?;
    bool(dest, arg.exclusive)?;
    nlm_lock(dest, arg.lock)?;
    bool(dest, arg.reclaim)?;
    u32(dest, arg.state)
}
