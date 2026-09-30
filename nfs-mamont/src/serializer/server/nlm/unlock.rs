//! Implements serialization for [`Nlm4UnlockRes`] structure.

use std::io;
use std::io::Write;

use crate::nlm::procedures::unlock::Nlm4UnlockRes;

use super::{cookie, stat};

/// Serializes an [`Nlm4UnlockRes`] as the XDR reply body for `NLMPROC4_UNLOCK`.
pub fn unlock_res(dest: &mut impl Write, res: Nlm4UnlockRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.stat)
}