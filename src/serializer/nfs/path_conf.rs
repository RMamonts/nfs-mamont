use std::io;
use std::io::Write;

use crate::serializer::nfs::files::file_attr;
use crate::serializer::{bool, option, u32};
use crate::vfs::path_conf;

pub fn result_ok(dest: &mut impl Write, arg: path_conf::Success) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))?;
    u32(dest, arg.link_max)?;
    u32(dest, arg.name_max)?;
    bool(dest, arg.no_trunc)?;
    bool(dest, arg.chown_restricted)?;
    bool(dest, arg.case_insensitive)?;
    bool(dest, arg.case_preserving)
}

pub fn result_fail(dest: &mut impl Write, arg: path_conf::Fail) -> io::Result<()> {
    option(dest, arg.file_attr, |attr, dest| file_attr(dest, attr))
}
