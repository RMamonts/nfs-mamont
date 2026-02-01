use std::io;
use std::io::Write;

use crate::serializer::nfs::file_handle;
use crate::vfs::get_attr;

#[allow(dead_code)]
pub fn get_attr_res_ok(dest: &mut impl Write, arg: get_attr::Success) -> io::Result<()> {
    file_handle(dest, arg.object)
}
