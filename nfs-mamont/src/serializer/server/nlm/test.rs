//! Implements serialization for [`Nlm4TestRes`] structure.

use std::io;
use std::io::Write;

use crate::nlm::procedures::test::Nlm4TestRes;
use crate::nlm::Nlm4Stats;
use crate::serializer::{u32, u64, vector};

use super::{cookie, stat};

/// Serializes an [`Nlm4TestRes`] as the XDR reply body for `NLMPROC4_TEST`.
///
/// The reply is an XDR union: when the status is `Denied` the current
/// lock holder information is included; otherwise only the status is
/// written.
pub fn test_res(dest: &mut impl Write, res: Nlm4TestRes) -> io::Result<()> {
    cookie(dest, res.cookie)?;
    stat(dest, res.test_stat.stat)?;
    if res.test_stat.stat == Nlm4Stats::Denied {
        let holder = match res.test_stat.holder {
            Some(holder) => holder,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Stat is Denied but holder is None",
                ))
            }
        };
        u32(dest, holder.exclusive as u32)?;
        u32(dest, holder.system_identifier as u32)?;
        vector(dest, holder.opaque_handle.as_bytes())?;
        u64(dest, holder.lock_offset)?;
        u64(dest, holder.lock_length)?;
    }
    Ok(())
}
