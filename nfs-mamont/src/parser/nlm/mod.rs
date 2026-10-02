//! Parsing of NLMv4 procedure arguments from incoming RPC calls.

use crate::consts::nlm;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::OpaqueHandle;
use crate::parser::nfsv3::file;
use crate::parser::primitive::{i32, string_max_size, u64, vector};
use crate::parser::{Error, Result};
use std::io::Read;

pub mod cancel;
pub mod lock;
pub mod test;
pub mod unlock;

/// Decodes the lock-owner identifier from an NLM request.
/// The wire encoding is a variable-length opaque capped at [`OPAQUE_HANDLE_SIZE`](nlm::OPAQUE_HANDLE_SIZE).
pub fn opaque_handle(src: &mut impl Read) -> Result<OpaqueHandle> {
    OpaqueHandle::new(vector(src)?).map_err(|_| Error::BadFileHandle)
}

/// Decodes the lock-arguments block shared by every LOCK/UNLOCK/TEST/CANCEL request.
pub fn parse_lock(src: &mut impl Read) -> Result<Nlm4Lock> {
    Nlm4Lock::new(
        string_max_size(src, nlm::LM_MAXSTRLEN)?,
        file::handle(src)?,
        opaque_handle(src)?,
        i32(src)?,
        u64(src)?,
        u64(src)?,
    )
    .map_err(|_| Error::BadFileHandle)
}

#[cfg(test)]
mod tests;
