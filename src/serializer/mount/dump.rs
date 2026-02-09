use std::io;
use std::io::Write;

use crate::mount;
use crate::mount::dump;
use crate::serializer::bool;
use crate::serializer::nfs::files::{file_name, file_path};

/// Serializes [`mount::MountEntry`] as an XDR `mountbody` linked list node.
pub fn mount_entry(dest: &mut impl Write, arg: mount::MountEntry) -> io::Result<()> {
    file_name(dest, arg.hostname)?;
    file_path(dest, arg.directory)
}

/// Serializes [`dump::Success`] as an XDR `mountbody` linked list node.
pub fn result_ok(dest: &mut impl Write, arg: dump::Success) -> io::Result<()> {
    for item in arg.mount_list {
        bool(dest, true)?;
        mount_entry(dest, item)?;
    }
    bool(dest, false)
}
