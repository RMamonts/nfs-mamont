//! MOUNT protocol (mountd) XDR serializers.
//!
//! This module serializes mountd reply bodies and helper structures (exports,
//! mount lists, groups) using the XDR rules shared by NFS/RPC.

use std::io;
use std::io::Write;

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::{file_name, file_path};
use crate::serializer::{bool, option, u32, vector_u32};
use crate::vfs::file;

#[allow(unused)]
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

/// Serializes `MountStat` as the XDR `mountstat3` enum discriminant.
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

pub struct MountResOk {
    handle: file::Handle,
    // maybe there should be something else? not u32 i mean
    auth_flavors: Vec<u32>,
}

/// Serializes `MountResOk` as the XDR `mountres3_ok` body.
pub fn mount_res_ok(dest: &mut impl Write, arg: MountResOk) -> io::Result<()> {
    file_handle(dest, arg.handle)?;
    vector_u32(dest, arg.auth_flavors)
}

/// Linked-list node for the XDR `mountlist`.
pub struct MountBody {
    ml_hostname: file::FileName,
    ml_directory: file::FilePath,
    ml_next: Option<Box<MountBody>>,
}

/// Serializes `MountBody` as an XDR `mountbody` linked list node.
pub fn mount_body(dest: &mut impl Write, arg: MountBody) -> io::Result<()> {
    file_name(dest, arg.ml_hostname)?;
    file_path(dest, arg.ml_directory)?;
    match arg.ml_next {
        Some(next) => mount_body(dest, *next),
        None => bool(dest, false),
    }
}

/// Linked-list node for the XDR `groups` list.
pub struct GroupNode {
    gr_name: file::FileName,
    groups: Option<Box<GroupNode>>,
}

/// Serializes `GroupNode` as an XDR `groupnode` linked list node.
pub fn group_node(dest: &mut impl Write, arg: GroupNode) -> io::Result<()> {
    file_name(dest, arg.gr_name)?;
    match arg.groups {
        None => bool(dest, false),
        Some(next) => group_node(dest, *next),
    }
}

/// Linked-list node for the XDR `exports` list.
pub struct ExportNode {
    ex_dir: file::FilePath,
    groups: Option<GroupNode>,
    exports: Option<Box<ExportNode>>,
}

/// Serializes `ExportNode` as an XDR `exportnode` linked list node.
pub fn export_node(dest: &mut impl Write, arg: ExportNode) -> io::Result<()> {
    file_path(dest, arg.ex_dir)?;
    option(dest, arg.groups, |arg, dest| group_node(dest, arg))?;
    match arg.exports {
        None => bool(dest, false),
        Some(next) => export_node(dest, *next),
    }
}
