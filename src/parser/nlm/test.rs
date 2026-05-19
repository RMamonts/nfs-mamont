//! Implements parsing for [`Nlm4TestArgs`] structure.

use crate::consts::nlm;
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::test::Nlm4TestArgs;
use crate::nlm::OpaqueHandle;
use crate::parser::nfsv3::file;
use crate::parser::primitive::{bool, i32, string_max_size, u64, array};
use crate::parser::{Error, Result};
use std::io::Read;

/// Parses the arguments for an NLMv4 `TEST` operation from the provided `Read` source.
pub fn test(src: &mut impl Read) -> Result<Nlm4TestArgs> {
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
        Err(_) => return Result::Err(Error::BadFileHandle),
    };

    Ok(Nlm4TestArgs { cookie: Cookie::new(u64(src)?), exclusive: bool(src)?, lock })
}
