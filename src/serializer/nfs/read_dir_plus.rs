use std::io;
use std::io::Write;

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_attr;
use crate::serializer::nfs::files::file_name;
use crate::serializer::{array, bool, option, u64};
use crate::vfs::read_dir_plus;
use crate::vfs::read_dir_plus::Entry;

pub fn entry(dest: &mut impl Write, entry: Entry) -> io::Result<()> {
    u64(dest, entry.file_id)?;
    file_name(dest, entry.file_name)?;
    u64(dest, entry.cookie)?;
    option(dest, entry.file_attr, |attr, dest| file_attr(dest, attr))?;
    option(dest, entry.file_handle, |handle, dest| file_handle(dest, handle))
}

pub fn dir_list_plus(dest: &mut impl Write, list: Vec<Entry>) -> io::Result<()> {
    for e in list {
        bool(dest, true)?;
        entry(dest, e)?;
    }
    bool(dest, false)
}

#[allow(dead_code)]
pub fn read_dir_plus_res_ok(dest: &mut impl Write, arg: read_dir_plus::Success) -> io::Result<()> {
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, attr))?;
    array(dest, arg.cookie_verifier.0)?;
    dir_list_plus(dest, arg.entries)
}

#[allow(dead_code)]
pub fn read_dir_plus_res_fail(dest: &mut impl Write, arg: read_dir_plus::Fail) -> io::Result<()> {
    option(dest, arg.dir_attr, |attr, dest| file_attr(dest, attr))
}
