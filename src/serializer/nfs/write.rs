//! XDR serializers for the NFSv3 `WRITE` procedure.

use std::io;
use std::io::Write;

use crate::serializer::nfs::files::wcc_data;
use crate::serializer::{array, u64, variant};
use crate::vfs::write;

/// Serializes [`write::StableHow`] as the XDR `stable_how` enum discriminant.
fn stable_how<S: Write>(dest: &mut S, how: write::StableHow) -> io::Result<()> {
    variant::<write::StableHow, S>(dest, how)
}

/// Serializes [`write::Success`] (WRITE3resok body) into XDR.
pub fn result_ok(dest: &mut impl Write, arg: write::Success) -> io::Result<()> {
    wcc_data(dest, arg.file_wcc)?;
    u64(dest, arg.count)?;
    stable_how(dest, arg.commited)?;
    array(dest, arg.verifier.0)
}

/// Serializes [`write::Fail`] (WRITE3resfail body) into XDR.
pub fn result_fail(dest: &mut impl Write, arg: write::Fail) -> io::Result<()> {
    wcc_data(dest, arg.wcc_data)
}
