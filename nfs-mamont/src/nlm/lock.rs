//! Defines NLMv4 basic lock structures.
//!
//! Contains [`Nlm4Lock`] and [`OpaqueHandle`] types used by lock procedures.


#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;

use crate::consts::nlm;
use crate::vfs;

use super::{Name, OpaqueHandle};

/// This structure describes a lock request.
#[cfg_attr(feature = "arbitrary", derive(Clone, Debug, PartialEq))]
pub struct Nlm4Lock {
    /// Name of the client host making the lock request.
    pub caller_name: Name,
    /// Handle to the file to lock.
    pub file_handle: vfs::file::Handle,
    /// Host or process that is making the request.
    pub opaque_handle: OpaqueHandle,
    /// PID of the process making the request.
    pub system_identifier: i32,
    /// Offset for the lock region.
    pub lock_offset: u64,
    /// Length of the blocking region. An l_len of 0 means "to end of file".
    pub lock_length: u64,
}

#[cfg(feature = "arbitrary")]
impl Arbitrary<'_> for Nlm4Lock {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let max_len = u.int_in_range(1..=nlm::LM_MAXSTRLEN)?;
        let mut caller_name = String::new();

        for _ in 0..max_len {
            match u.int_in_range(0u32..=25u32) {
                Ok(idx) => {
                    caller_name.push((b'a' + idx as u8) as char);
                }
                Err(_) => break,
            }
        }
        Ok(Nlm4Lock {
            caller_name: Name::new(caller_name).map_err(|_| arbitrary::Error::IncorrectFormat)?,
            file_handle: u.arbitrary()?,
            opaque_handle: u.arbitrary()?,
            system_identifier: u.arbitrary()?,
            lock_offset: u.arbitrary()?,
            lock_length: u.arbitrary()?,
        })
    }
}

