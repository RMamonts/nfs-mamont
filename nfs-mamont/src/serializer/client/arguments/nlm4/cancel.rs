use std::io;
use std::io::Write;

use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::cancel;
use crate::serializer::files::file_handle;
use crate::serializer::server::nlm::{cookie, opaque_handle};
use crate::serializer::{bool, string, u32, u64};

/// Serializes the arguments [`cancel::Nlm4CancelArgs`] for a Mount `CANCEL` operation to the provided `Write` destination.
pub fn cancel_args(dest: &mut impl Write, arg: cancel::Nlm4CancelArgs) -> io::Result<()> {
    cookie(dest, arg.cookie)?;
    bool(dest, arg.block)?;
    bool(dest, arg.exclusive)?;
    nlm_lock(dest, arg.lock)
}

/// Serializes [`Nlm4Lock`] to the provided `Write` destination.
pub fn nlm_lock(dest: &mut impl Write, arg: Nlm4Lock) -> io::Result<()> {
    string(dest, &arg.caller_name)?;
    file_handle(dest, arg.file_handle)?;
    opaque_handle(dest, arg.opaque_handle)?;
    u32(dest, arg.system_identifier as u32)?;
    u64(dest, arg.lock_offset)?;
    u64(dest, arg.lock_length)
}
