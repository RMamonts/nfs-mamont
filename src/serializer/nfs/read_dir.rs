use std::io;
use std::io::Write;

use crate::serializer::nfs::files::{file_attr, file_name};
use crate::serializer::{array, bool, option, u64};
use crate::vfs::read_dir;
use crate::vfs::read_dir::Entry;

fn entry(dest: &mut impl Write, entry: Entry) -> io::Result<()> {
    u64(dest, entry.file_id)?;
    file_name(dest, entry.file_name)?;
    u64(dest, entry.cookie)
}

fn dir_list(dest: &mut impl Write, list: Vec<Entry>) -> io::Result<()> {
    for e in list {
        bool(dest, true)?;
        entry(dest, e)?;
    }
    bool(dest, false)
}

pub fn result_ok(dest: &mut impl Write, arg: read_dir::Success) -> io::Result<()> {
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, attr))?;
    array(dest, arg.cookie_verifier.0)?;
    dir_list(dest, arg.entries)
}

pub fn result_fail(dest: &mut impl Write, arg: read_dir::Fail) -> io::Result<()> {
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, attr))
}
