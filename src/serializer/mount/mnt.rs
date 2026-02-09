use std::io;
use std::io::Write;

use crate::mount::mnt;
use crate::mount::mnt::AuthFlavor;
use crate::serializer::nfs::file_handle;

/// Serializes [`AuthFlavor`] as the XDR `mountres3_ok` body.
pub fn auth_flavor(_dest: &mut impl Write, _vec: Vec<AuthFlavor>) -> io::Result<()> {
    todo!()
}

/// Serializes [`mnt::Success`] as the XDR `mountres3_ok` body.
pub fn result_ok(dest: &mut impl Write, arg: mnt::Success) -> io::Result<()> {
    file_handle(dest, arg.file_handle)?;
    auth_flavor(dest, arg.auth_flavors)
}
