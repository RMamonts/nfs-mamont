//! MOUNT protocol (mount) XDR serializers.
//!
//! This module serializes mount reply bodies and helper structures (exports,
//! mount lists, groups) using the XDR rules shared by NFS/RPC.

use std::io;
use std::io::Write;

use crate::mount::MntError;
use crate::serializer::variant;

pub mod dump;
pub mod export;
pub mod mnt;

/// Serializes [`MntError`] as the XDR `mountstat3` enum discriminant.
pub fn mount_stat<S: Write>(dest: &mut S, status: MntError) -> io::Result<()> {
    variant::<MntError, S>(dest, status)
}
