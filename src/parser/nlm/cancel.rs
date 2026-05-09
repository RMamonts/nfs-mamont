//! Implements parsing for [`Nlm4CancelArgs`] structure.

use crate::consts::nlm;
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::cancel::Nlm4CancelArgs;
use crate::nlm::OpaqueHandle;
use crate::parser::nfsv3::file;
use crate::parser::primitive::{bool, i32, string_max_size, u64, vector};
use crate::parser::{Error, Result};
use std::io::Read;

/// Parses the arguments for an NLMv4 `CANCEL` operation from the provided `Read` source.
pub fn cancel(src: &mut impl Read) -> Result<Nlm4CancelArgs> {
    let caller_name = string_max_size(src, nlm::LM_MAXSTRLEN)?;
    let lock = match Nlm4Lock::new(
        caller_name,
        file::handle(src)?,
        OpaqueHandle::new(vector(src)?),
        i32(src)?,
        u64(src)?,
        u64(src)?,
    ) {
        Ok(l) => l,
        Err(_) => return Result::Err(Error::BadFileHandle),
    };

    Ok(Nlm4CancelArgs {
        cookie: Cookie::new(u64(src)?),
        block: bool(src)?,
        exclusive: bool(src)?,
        lock,
    })
}
