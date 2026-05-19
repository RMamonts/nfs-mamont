//! Implements parsing for [`Nlm4UnlockArgs`] structure.

use crate::consts::nlm;
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::unlock::Nlm4UnlockArgs;
use crate::nlm::OpaqueHandle;
use crate::parser::nfsv3::file;
use crate::parser::primitive::{i32, string_max_size, u64, array};
use crate::parser::{Error, Result};
use std::io::Read;

/// Parses the arguments for an NLMv4 `UNLOCK` operation from the provided `Read` source.
pub fn unlock(src: &mut impl Read) -> Result<Nlm4UnlockArgs> {
    let caller_name = string_max_size(src, nlm::LM_MAXSTRLEN)?;
    let lock = match Nlm4Lock::new(
        caller_name,
        file::handle(src)?,
        OpaqueHandle::new(array(src)?),
        i32(src)?,
        u64(src)?,
        u64(src)?,
    ) {
        Ok(l) => l,
        Err(_) => return Err(Error::BadFileHandle),
    };

    let cookie = Cookie::new(u64(src)?);

    Ok(Nlm4UnlockArgs { cookie, lock })
}
