//! MOUNT protocol (mount) XDR serializers.
//!
//! This module serializes mount reply bodies and helper structures (exports,
//! mount lists, groups) using the XDR rules shared by NFS/RPC.

use std::io;
use std::io::Write;

use crate::interface::mount::mnt::Fail;
use crate::serializer::variant;

pub mod dump;
pub mod export;
pub mod mnt;

/// Serializes [`Fail`] as the XDR `mountstat3` enum discriminant.
pub fn mount_stat(dest: &mut impl Write, status: Fail) -> io::Result<()> {
    variant::<Fail>(dest, status)
}
