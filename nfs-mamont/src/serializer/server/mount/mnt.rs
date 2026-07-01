use std::io;
use std::io::Write;

use crate::mount::mnt;
use crate::rpc::AuthFlavor;
use crate::serializer::files::file_handle;
use crate::serializer::{usize_as_u32, variant};

/// Serializes [`AuthFlavor`] as the XDR `mountres3_ok` body.
pub fn auth_flavor_vec(dest: &mut impl Write, vec: Vec<AuthFlavor>) -> io::Result<()> {
    usize_as_u32(dest, vec.len())?;
    for auth in vec {
        variant::<AuthFlavor>(dest, auth)?;
    }
    Ok(())
}

/// Serializes [`mnt::Success`] as the XDR `mountres3_ok` body.
pub fn result_ok(dest: &mut impl Write, arg: mnt::Success) -> io::Result<()> {
    file_handle(dest, arg.file_handle)?;
    auth_flavor_vec(dest, arg.auth_flavors)
}
