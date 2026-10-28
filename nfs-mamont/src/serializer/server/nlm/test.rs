//! Implements serialization for [`Nlm4TestRes`] structure.

use std::io;
use std::io::Write;

use crate::nlm::procedures::test::Nlm4TestRes;
use crate::nlm::Nlm4Stats;

use super::{cookie, holder, stat};

/// Serializes an [`Nlm4TestRes`] as the XDR reply body for `NLMPROC4_TEST`.
///
/// The reply is an XDR union: when the status is `Denied` the current
/// lock holder information is included; otherwise only the status is
/// written.
pub fn test_res(dest: &mut impl Write, res: Nlm4TestRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.test_stat.stat)?;
    match (res.test_stat.stat, res.test_stat.holder) {
        (Nlm4Stats::Denied, Some(nlm_holder)) => holder(dest, nlm_holder),
        (Nlm4Stats::Denied, None) | (_, Some(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "holder should be present only with Denied status",
        )),
        (_, None) => Ok(()),
    }
}
