use std::io::{Result, Write};

use crate::serializer::files::file_handle;
use crate::serializer::{option, u32, u64, variant};
use crate::vfs::file::Time;
use crate::vfs::set_attr::{Args, Guard, NewAttr, SetTime, SetTimeMode};

/// Serializes [`Time`].
fn serialize_nfs_time(dest: &mut impl Write, time: Time) -> Result<()> {
    u32(dest, time.seconds).and_then(|_| u32(dest, time.nanos))
}

/// Serializes [`SetTime`].
fn serialize_set_time(dest: &mut impl Write, set_time: SetTime) -> Result<()> {
    match set_time {
        SetTime::DontChange => variant(dest, SetTimeMode::DontChange),
        SetTime::ToServer => variant(dest, SetTimeMode::ToServer),
        SetTime::ToClient(time) => {
            variant(dest, SetTimeMode::ToClient).and_then(|_| serialize_nfs_time(dest, time))
        }
    }
}

/// Serializes [`NewAttr`].
pub fn serialize_new_attr(dest: &mut impl Write, attr: NewAttr) -> Result<()> {
    option(dest, attr.mode, |n, dest| u32(dest, n))
        .and_then(|_| option(dest, attr.uid, |n, dest| u32(dest, n)))
        .and_then(|_| option(dest, attr.gid, |n, dest| u32(dest, n)))
        .and_then(|_| option(dest, attr.size, |n, dest| u64(dest, n)))
        .and_then(|_| serialize_set_time(dest, attr.atime))
        .and_then(|_| serialize_set_time(dest, attr.mtime))
}

/// Serializes [`Guard`].
fn serialize_guard(dest: &mut impl Write, guard: Guard) -> Result<()> {
    serialize_nfs_time(dest, guard.ctime)
}

/// Serializes the arguments [`Args`]for an NFSv3 `SETATTR` operation to the provided `Write` destination.
pub fn set_attr_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.file)
        .and_then(|_| serialize_new_attr(dest, arg.new_attr))
        .and_then(|_| option(dest, arg.guard, |arg, dest| serialize_guard(dest, arg)))
}
