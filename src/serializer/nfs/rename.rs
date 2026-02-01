use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::vfs::rename;

#[allow(dead_code)]
pub fn rename_res_ok(dest: &mut impl Write, arg: rename::Success) -> io::Result<()> {
    wcc_data(dest, arg.from_dir_wcc)?;
    wcc_data(dest, arg.to_dir_wcc)
}

#[allow(dead_code)]
pub fn rename_res_fail(dest: &mut impl Write, arg: rename::Fail) -> io::Result<()> {
    wcc_data(dest, arg.from_dir_wcc)?;
    wcc_data(dest, arg.to_dir_wcc)
}
