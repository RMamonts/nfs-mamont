//! Parsing of NLMv4 procedure arguments from incoming RPC calls.

use std::io::Read;

use crate::consts::nlm;
use crate::consts::nlm::OPAQUE_HANDLE_SIZE;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::{Name, OpaqueHandle};
use crate::parser::nfsv3::file;
use crate::parser::primitive::{i32, string_max_size, u64, vec_max_size};
use crate::parser::{Error, Result};

pub mod cancel;
pub mod lock;
pub mod test;
pub mod unlock;

/// Decodes the lock-owner identifier from an NLM request.
/// The wire encoding is a variable-length opaque capped at [`OPAQUE_HANDLE_SIZE`](nlm::OPAQUE_HANDLE_SIZE).
pub fn opaque_handle(src: &mut impl Read) -> Result<OpaqueHandle> {
    OpaqueHandle::new(vec_max_size(src, OPAQUE_HANDLE_SIZE)?).map_err(Error::IO)
}

pub fn caller_name(src: &mut impl Read) -> Result<Name> {
    Name::new(string_max_size(src, nlm::LM_MAXSTRLEN)?).map_err(Error::IO)
}

/// Decodes the lock-arguments block shared by every LOCK/UNLOCK/TEST/CANCEL request.
pub fn parse_lock(src: &mut impl Read) -> Result<Nlm4Lock> {
    Ok(Nlm4Lock::new(
        caller_name(src)?,
        file::handle(src)?,
        opaque_handle(src)?,
        i32(src)?,
        u64(src)?,
        u64(src)?,
    ))
}

#[cfg(test)]
mod tests;
