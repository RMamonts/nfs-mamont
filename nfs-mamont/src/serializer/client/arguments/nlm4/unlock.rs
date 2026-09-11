use std::io;
use std::io::Write;

use crate::nlm::procedures::unlock;
use crate::serializer::client::arguments::nlm4::cancel::nlm_lock;
use crate::serializer::server::nlm::cookie;

/// Serializes the arguments [`unlock::Nlm4UnlockArgs`] for a Mount `UNLOCK` operation to the provided `Write` destination.
pub fn unlock_args(dest: &mut impl Write, arg: unlock::Nlm4UnlockArgs) -> io::Result<()> {
    cookie(dest, arg.cookie)?;
    nlm_lock(dest, arg.lock)
}
