use std::io;
use std::io::Write;

use crate::nfsv3::nfs_fh3;
use crate::serializer::nfs::nfs_fh3;
use crate::serializer::nfs::results::{filename3, nfspath3};
use crate::serializer::{bool, option, u32, vector_u32};
use crate::vfs::{FileHandle, FileName, FsPath};

#[allow(dead_code)]
pub enum MountStat {
    OK = 0,              /* no error */
    Perm = 1,            /* Not owner */
    NoEnt = 2,           /* No such file or directory */
    IO = 5,              /* I/O error */
    Access = 13,         /* Permission denied */
    NotDir = 20,         /* Not a directory */
    Invalid = 22,        /* Invalid argument */
    NameTooLong = 63,    /* Filename too long */
    NotSupp = 10004,     /* Operation not supported */
    ServerFault = 10006, /* A failure on the server */
}

#[allow(dead_code)]
pub fn mount_stat(dest: &mut impl Write, status: MountStat) -> io::Result<()> {
    let numb = match status {
        MountStat::OK => 0,
        MountStat::Perm => 1,
        MountStat::NoEnt => 2,
        MountStat::IO => 5,
        MountStat::Access => 13,
        MountStat::NotDir => 20,
        MountStat::Invalid => 22,
        MountStat::NameTooLong => 63,
        MountStat::NotSupp => 10004,
        MountStat::ServerFault => 10006,
    };
    u32(dest, numb)
}

#[allow(dead_code)]
pub struct MountResOk {
    fhandle: FileHandle,
    // maybe there should be something else? not u32 i mean
    auth_flavors: Vec<u32>,
}

#[allow(dead_code)]
fn mount_res_ok(dest: &mut impl Write, arg: MountResOk) -> io::Result<()> {
    nfs_fh3(dest, nfs_fh3 { data: arg.fhandle.0 })?;
    vector_u32(dest, arg.auth_flavors)
}

pub struct MountBody {
    ml_hostname: FileName,
    ml_directory: FsPath,
    ml_next: Option<Box<MountBody>>,
}

#[allow(dead_code)]
pub fn mount_body(dest: &mut dyn Write, arg: MountBody) -> io::Result<()> {
    filename3(dest, arg.ml_hostname)?;
    nfspath3(dest, arg.ml_directory)?;
    match arg.ml_next {
        Some(next) => mount_body(dest, *next),
        None => bool(dest, false),
    }
}

#[allow(dead_code)]
pub struct GroupNode {
    gr_name: FileName,
    groups: Option<Box<GroupNode>>,
}

#[allow(dead_code)]
pub fn group_node(dest: &mut dyn Write, arg: GroupNode) -> io::Result<()> {
    filename3(dest, arg.gr_name)?;
    match arg.groups {
        None => bool(dest, false),
        Some(next) => group_node(dest, *next),
    }
}

#[allow(dead_code)]
pub struct ExportNode {
    ex_dir: FsPath,
    groups: Option<GroupNode>,
    exports: Option<Box<ExportNode>>,
}

#[allow(dead_code)]
pub fn export_node(dest: &mut dyn Write, arg: ExportNode) -> io::Result<()> {
    nfspath3(dest, arg.ex_dir)?;
    option(dest, arg.groups, |arg, dest| group_node(dest, arg))?;
    match arg.exports {
        None => bool(dest, false),
        Some(next) => export_node(dest, *next),
    }
}
