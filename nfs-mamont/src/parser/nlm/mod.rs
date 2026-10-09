//! Parsing of NLMv4 procedure arguments from incoming RPC calls.

use std::io::Read;

use crate::consts::nlm;
use crate::consts::nlm::OPAQUE_HANDLE_SIZE;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::{Nlm4Stats, Name, OpaqueHandle};
use crate::parser::nfsv3::file;
use crate::parser::primitive::u32;
use crate::parser::primitive::{i32, string_max_size, u64, vector, vec_max_size};
use crate::parser::{Error, Result};

pub mod cancel;
pub mod granted;
pub mod lock;
pub mod test;
pub mod unlock;

/// Parses a Nlm4Stats enum from the provided Read source.
pub fn nlm_stat(src: &mut impl Read) -> Result<Nlm4Stats> {
    match u32(src)? {
        0 => Ok(Nlm4Stats::Granted),
        1 => Ok(Nlm4Stats::Denied),
        2 => Ok(Nlm4Stats::DeniedNolocks),
        3 => Ok(Nlm4Stats::Blocked),
        4 => Ok(Nlm4Stats::DeniedGracePeriod),
        5 => Ok(Nlm4Stats::Deadlock),
        6 => Ok(Nlm4Stats::Rofs),
        7 => Ok(Nlm4Stats::StaleFh),
        8 => Ok(Nlm4Stats::Fbig),
        9 => Ok(Nlm4Stats::Failed),
        _ => Err(Error::EnumDiscMismatch),
    }
}

/// Decodes the lock-owner identifier from an NLM request.
/// The wire encoding is a variable-length opaque capped at [`OPAQUE_HANDLE_SIZE`].
pub fn opaque_handle(src: &mut impl Read) -> Result<OpaqueHandle> {
    OpaqueHandle::new(vec_max_size(src, OPAQUE_HANDLE_SIZE)?).map_err(Error::IO)
}

pub fn caller_name(src: &mut impl Read) -> Result<Name> {
    Name::new(string_max_size(src, nlm::LM_MAXSTRLEN)?).map_err(Error::IO)
}

/// Decodes the lock-arguments block shared by every LOCK/UNLOCK/TEST/CANCEL request.
pub fn parse_lock(src: &mut impl Read) -> Result<Nlm4Lock> {
    Ok(Nlm4Lock {
        caller_name: caller_name(src)?,
        file_handle: file::handle(src)?,
        opaque_handle: opaque_handle(src)?,
        system_identifier: i32(src)?,
        lock_offset: u64(src)?,
        lock_length: u64(src)?,
    })
}

#[cfg(test)]
mod tests;
