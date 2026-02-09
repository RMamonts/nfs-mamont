use std::io;
use std::io::Write;

use crate::serializer::nfs::file_handle;
use crate::vfs::get_attr;

pub fn result_ok(dest: &mut impl Write, arg: get_attr::Success) -> io::Result<()> {
    file_handle(dest, arg.object)
}
