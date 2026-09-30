//! Implements serialization for [`Nlm4CancelRes`] structure.

use std::io;
use std::io::Write;

use crate::nlm::procedures::cancel::Nlm4CancelRes;

use super::{cookie, stat};

/// Serializes an [`Nlm4CancelRes`] as the XDR reply body for `NLMPROC4_CANCEL`.
pub fn cancel_res(dest: &mut impl Write, res: Nlm4CancelRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.stat)
}
