use std::io;
use std::io::Write;

use crate::mount;
use crate::mount::{export, MOUNT_HOST_NAME_LEN};
use crate::serializer::files::file_path;
use crate::serializer::{bool, string_max_size};

/// Serializes [`mount::ExportEntry`] as an XDR `groupnode` linked list node.
pub fn export_entry(dest: &mut impl Write, arg: mount::ExportEntry) -> io::Result<()> {
    file_path(dest, arg.directory)?;
    for item in arg.names {
        bool(dest, true)?;
        string_max_size(dest, item, MOUNT_HOST_NAME_LEN)?;
    }
    bool(dest, false)
}

/// Serializes [`export::Success`] as an XDR `exportnode` linked list node.
pub fn result_ok(dest: &mut impl Write, arg: export::Success) -> io::Result<()> {
    for item in arg.exports {
        bool(dest, true)?;
        export_entry(dest, item)?;
    }
    bool(dest, false)
}
