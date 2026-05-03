//! Implements parsing for [`commit::Args`] structure.

use std::io::Read;
use crate::consts::nlm;
use crate::parser::nfsv3::file;
use crate::parser::primitive::{bool, u64, u32, string_max_size, vector};
use crate::parser::Result;
use crate::nlm::procedures::lock::Nlm4LockArgs;
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::OpaqueHandle;

/// Parses the arguments for an NLMv4 `LOCK` operation from the provided `Read` source.
pub fn lock(src: &mut impl Read) -> Result<Nlm4LockArgs> {
    let caller_name = string_max_size(src, nlm::LM_MAXSTRLEN)?;
    Ok(Nlm4LockArgs {
        cookie: Cookie::new(u64(src)?),
        block: bool(src)?,
        exclusive: bool(src)?,
        lock: Nlm4Lock::new(
            caller_name,
            file::handle(src)?,
            OpaqueHandle::new(vector(src)?),
            u32(src)?,
            u64(src)?,
            u64(src)?,
        ).unwrap(), // We can use "?", but then you will lose information about the constructor error
        reclaim: bool(src)?,
        state: u32(src)?,
    })
}