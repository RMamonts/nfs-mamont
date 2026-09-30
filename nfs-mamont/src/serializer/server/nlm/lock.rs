//! Implements serialization for [`Nlm4LockRes`] structure.

use std::io;
use std::io::Write;

use crate::nlm::procedures::lock::Nlm4LockRes;

use super::{cookie, stat};

/// Serializes an [`Nlm4LockRes`] as the XDR reply body for `NLMPROC4_LOCK`.
pub fn lock_res(dest: &mut impl Write, res: Nlm4LockRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.stat)
}
