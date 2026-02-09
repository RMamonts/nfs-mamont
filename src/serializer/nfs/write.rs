use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::serializer::{array, u64, variant};
use crate::vfs::write;

fn stable_how<S: Write>(dest: &mut S, how: write::StableHow) -> io::Result<()> {
    variant::<write::StableHow, S>(dest, how)
}

pub fn result_ok(dest: &mut impl Write, arg: write::Success) -> io::Result<()> {
    wcc_data(dest, arg.file_wcc)?;
    u64(dest, arg.count)?;
    stable_how(dest, arg.commited)?;
    array(dest, arg.verifier.0)
}

pub fn result_fail(dest: &mut impl Write, arg: write::Fail) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}
