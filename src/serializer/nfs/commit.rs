use std::io;
use std::io::Write;

use crate::serializer::array;
use crate::serializer::nfs::files::wcc_data;
use crate::vfs::commit;

#[allow(dead_code)]
pub fn commit_res_ok(dest: &mut impl Write, arg: commit::Success) -> io::Result<()> {
    wcc_data(dest, arg.file_wcc)?;
    array(dest, arg.verifier.0)
}

#[allow(dead_code)]
pub fn commit_res_fail(dest: &mut impl Write, arg: commit::Fail) -> io::Result<()> {
    wcc_data(dest, arg.file_wcc)
}
