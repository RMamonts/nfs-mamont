use std::io;
use std::io::Write;

use crate::nlm::procedures::test;
use crate::serializer::bool;
use crate::serializer::client::arguments::nlm4::cancel::nlm_lock;
use crate::serializer::server::nlm::cookie;

/// Serializes the arguments [`test::Nlm4TestArgs`] for a Mount `TEST` operation to the provided `Write` destination.
pub fn test_args(dest: &mut impl Write, arg: test::Nlm4TestArgs) -> io::Result<()> {
    cookie(dest, arg.cookie)?;
    bool(dest, arg.exclusive)?;
    nlm_lock(dest, arg.lock)
}
